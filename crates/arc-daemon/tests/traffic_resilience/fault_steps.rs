//! The steps of the NFR-009 driver: one method per fault kind.
use super::*;

impl Run {
    /// Jobs holding a permit: running, or orphaned with a live child.
    fn holders(&self) -> Vec<usize> {
        let mut holders = self.live(Phase::Running);
        holders.extend(self.live(Phase::Orphan));
        holders
    }

    fn release_holder(&mut self, index: usize) {
        if self.jobs[index].phase == Phase::Running {
            self.release(index);
        } else {
            self.release_orphan(index);
        }
    }

    fn release_any(&mut self) {
        let holders = self.holders();
        if holders.is_empty() {
            self.start(true);
        } else {
            let index = self.rng.pick(&holders);
            self.release_holder(index);
        }
    }

    /// A wrapper with no tool on its PATH. Its one spawn is tried when its
    /// turn comes, so the holders ahead of it end first. The spawn fails, the
    /// permit it was granted is released and no child ever starts.
    fn failed_spawn(&mut self) {
        let index = self.start(false);
        while self.jobs[index].phase == Phase::Queued {
            let holders = self.holders();
            let holder = self.rng.pick(&holders);
            self.release_holder(holder);
            self.settle();
        }
        assert_eq!(self.jobs[index].job.starts(), 0);
    }

    fn wrapper_killed_while_running(&mut self) {
        while self.live(Phase::Running).is_empty() {
            match self.live(Phase::Orphan).first() {
                Some(orphan) => self.release_orphan(*orphan),
                None => {
                    self.start(true);
                }
            }
            self.settle();
        }
        let index = self.rng.pick(&self.live(Phase::Running));
        let tracked = &mut self.jobs[index];
        tracked.job.kill_wrapper();
        tracked.phase = Phase::Orphan;
        self.tally.orphaned += 1;
        // The child is alive and recorded: it keeps the job and the permit.
        assert_eq!(
            self.reconciled(index),
            ("RUNNING".to_owned(), Some("ACTIVE".to_owned()))
        );
        assert!(crate::fixture::alive(
            self.jobs[index].job.child.as_ref().unwrap()
        ));
    }

    fn wrapper_killed_while_queued(&mut self) {
        while self.live(Phase::Queued).is_empty() {
            // A second job of a class whose window is taken has to queue.
            let (subcommand, class) = match self.holders().first() {
                Some(holder) => {
                    let class = self.jobs[*holder].class;
                    *CLASSES.iter().find(|(_, c)| *c == class).unwrap()
                }
                None => self.rng.pick(&CLASSES),
            };
            self.start_class(subcommand, class, true);
        }
        let index = self.rng.pick(&self.live(Phase::Queued));
        let db = self.w.db();
        let tracked = &mut self.jobs[index];
        assert_eq!(tracked.job.state(&db).as_deref(), Some("QUEUED"));
        tracked.job.kill_wrapper();
        tracked.phase = Phase::Done;
        assert_eq!(tracked.job.starts(), 0, "a queued job started its child");
        self.tally.killed_queued += 1;
        // Seeded: reconcile now, or leave the dead entry for a later poll,
        // daemon start or the final status read to find.
        if self.rng.below(2) == 0 {
            let (entry, permit) = self.reconciled(index);
            assert!(
                entry == "ABANDONED" && permit.is_none(),
                "a wrapper killed while queued: {entry} {permit:?}"
            );
        }
    }

    /// SIGKILL the daemon and start it again while at least one job holds a
    /// permit. Running jobs and orphans keep their permits and children. A
    /// queued wrapper finds no daemon at its next poll and fails open: it
    /// starts its child once, without a permit. With the seed's choice an
    /// orphan's child exits during the outage, so the daemon's start alone
    /// has to release its permit.
    fn daemon_kill_and_restart(&mut self) {
        if self.holders().is_empty() {
            self.start(true);
        }
        self.w.kill();
        let orphans = self.live(Phase::Orphan);
        let exited =
            (!orphans.is_empty() && self.rng.below(3) != 0).then(|| self.rng.pick(&orphans));
        if let Some(index) = exited {
            self.jobs[index].job.release();
            self.jobs[index].job.child_gone();
            self.jobs[index].phase = Phase::Done;
        }
        let queued = self.live(Phase::Queued);
        for index in &queued {
            let tracked = &mut self.jobs[*index];
            tracked.job.reach_barrier();
            tracked.starts = 1;
            self.tally.failed_open += 1;
            let stderr = tracked.job.diagnostics();
            assert!(stderr.contains("TRACKING_UNCERTAIN\n"), "{stderr}");
        }
        self.w.boot();
        // Startup reconciliation only: no request has been made yet.
        self.observe("daemon restarted");
        let db = self.w.db();
        self.monitor.owners_alive(&db, "daemon restarted");
        if let Some(index) = exited {
            let id = self.jobs[index].job.id(&db).unwrap();
            let states: (String, String) = db
                .query_row(
                    "SELECT t.state,p.state FROM traffic_entries t JOIN permits p \
                     ON p.job_id=t.job_id WHERE t.job_id=?1",
                    [&id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                states,
                ("ABANDONED".to_owned(), "RELEASED".to_owned()),
                "the daemon start releases the permit of a gone child"
            );
            self.tally.released_at_daemon_start += 1;
        }
        for index in self.holders() {
            let tracked = &mut self.jobs[index];
            let child = tracked.job.child.clone().unwrap();
            assert!(crate::fixture::alive(&child));
            assert_eq!(
                tracked.job.recorded_child(&db),
                Some((child.pid, child.start_token))
            );
            assert_eq!(tracked.job.state(&db).as_deref(), Some("RUNNING"));
            assert_eq!(tracked.job.starts(), 1, "the restart replayed a command");
            self.tally.kept_across_daemon_kill += 1;
        }
        // The fail-open children end: each wrapper exits 0 and holds nothing.
        for index in queued {
            let tracked = &mut self.jobs[index];
            tracked.job.release();
            let status = tracked.job.wrapper.wait("fail-open wrapper").unwrap();
            assert!(
                status.success(),
                "{status:?}: {}",
                tracked.job.diagnostics()
            );
            tracked.phase = Phase::Done;
            let (entry, permit) = self.reconciled(index);
            assert!(
                !matches!(
                    entry.as_str(),
                    "QUEUED" | "RESERVED" | "RUNNING" | "RETAINED"
                ) && permit.is_none(),
                "a fail-open job: {entry} {permit:?}"
            );
        }
    }

    /// A job outside the queue, through the fault proxy: the wrapper's
    /// registration or claim is dropped, answered busy, or committed with its
    /// reply withheld. The proxy's own record shows the fault and the wrapper's
    /// retry. The command still runs exactly once, and no second job row and
    /// no permit appear.
    fn job_through_proxy(&mut self, mode: &str, evidence: &str) {
        self.tally.proxied += 1;
        let n = self.tally.proxied;
        let fault = Fault::at(&self.w.f, mode, &format!("fault-{n}.sock"));
        let marker = self.w.f.root.join(format!("proxied-{n}.starts"));
        let stderr = self.w.f.root.join(format!("proxied-{n}.stderr"));
        let mut command = self.w.s.arcctl(&self.w.f, &self.w.project);
        command
            .args(["exec", "--", "tool", "run"])
            .env(arc_core::ARC_SOCKET_PATH, &fault.path)
            .env("ARC_FIXTURE_MODE", "success")
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::fs::File::create(&stderr).unwrap());
        let mut wrapper = crate::runtime::child_watchdog::Owned::spawn(&mut command).unwrap();
        let status = wrapper.wait("job through the fault proxy").unwrap();
        let report = fault.finish();
        let stderr = crate::fixture::file(&stderr);
        assert!(status.success(), "{mode}: {status:?}: {stderr}");
        assert_eq!(crate::journey::starts(&marker), 1, "{mode}: {stderr}");
        let events: Vec<&str> = report
            .events
            .iter()
            .filter_map(|event| event["event"].as_str())
            .collect();
        assert!(events.contains(&evidence), "{mode}: {events:?}");
        let action = if mode.ends_with("claim") {
            "CLAIM"
        } else {
            "REGISTER"
        };
        assert!(
            report.counts.get(action).is_some_and(|sent| *sent >= 2),
            "{mode}: the wrapper did not retry: {:?}",
            report.counts
        );
        let (rows, permits): (i64, i64) = self
            .w
            .db()
            .query_row(
                "SELECT count(*),(SELECT count(*) FROM permits p JOIN job_lifecycle o \
                 ON o.job_id=p.job_id WHERE o.observer_pid=?1) FROM job_lifecycle \
                 WHERE observer_pid=?1",
                [wrapper.child.id()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(
            rows <= 1 && permits == 0,
            "{mode}: {rows} job rows, {permits} permits"
        );
    }

    /// A scheduled job driven over the daemon's socket by this process, so a
    /// reply can be lost after its commit and a request repeated at will.
    fn protocol_job(&mut self, lose_replies: bool, repeats: usize) {
        self.protocol_jobs += 1;
        let (subcommand, class) = self.rng.pick(&CLASSES);
        let db = self.w.db();
        // The driver's own reading of the policy says whether it can start.
        let admitted = self.live(Phase::Queued).is_empty() && self.monitor.admits(&db, class);
        let granted = protocol::job(
            &self.w.f,
            &db,
            self.protocol_jobs,
            subcommand,
            lose_replies,
            repeats,
            &mut |at| self.monitor.observe(&db, at),
        );
        assert_eq!(granted, admitted, "protocol job {}", self.protocol_jobs);
        if granted {
            self.tally.protocol_granted += 1;
        } else {
            self.tally.protocol_cancelled += 1;
        }
    }

    pub(super) fn apply(&mut self, step: Step) {
        match step {
            Step::Start if self.live(Phase::Queued).len() + self.holders().len() < MAX_LIVE => {
                self.start(true);
            }
            Step::Start | Step::Release => self.release_any(),
            Step::Fault(Kind::FailedSpawn) => self.failed_spawn(),
            Step::Fault(Kind::WrapperKilledQueued) => self.wrapper_killed_while_queued(),
            Step::Fault(Kind::WrapperKilledRunning) => self.wrapper_killed_while_running(),
            Step::Fault(Kind::DaemonKillRestart) => self.daemon_kill_and_restart(),
            Step::Fault(Kind::RetriedRequest) => {
                // The request is dropped before the daemon sees it, or the
                // daemon's answer is DB_BUSY: either way the wrapper retries.
                let (mode, evidence) = self.rng.pick(&[
                    ("disconnect-register", "disconnected-before-commit"),
                    ("disconnect-claim", "disconnected-before-commit"),
                    ("busy-register", "injected-db-busy"),
                    ("busy-claim", "injected-db-busy"),
                ]);
                self.job_through_proxy(mode, evidence);
                let repeats = 1 + self.rng.below(3);
                self.protocol_job(false, repeats);
            }
            Step::Fault(Kind::LostReply) => {
                let mode = self.rng.pick(&["committed-register", "committed-claim"]);
                self.job_through_proxy(mode, "backend-ack-withheld");
                self.protocol_job(true, 1);
            }
        }
        self.settle();
    }
}
