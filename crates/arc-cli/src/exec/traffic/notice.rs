use super::super::preparation::{write_notice, NoticeWrite};
use arc_ipc::jobs::JobId;

/// Write once without changing the inherited descriptor's shared status flags.
#[derive(Default)]
pub(super) struct NoticeState {
    pub(super) last: Option<bool>,
    pub(super) uncertain: bool,
    pressure: Option<bool>,
    failed_writes: u8,
}

pub(super) fn announce_if_needed(
    state: &mut NoticeState,
    pressure: bool,
    emit: impl FnOnce(bool) -> NoticeWrite,
) {
    if state.pressure != Some(pressure) {
        state.pressure = Some(pressure);
        state.failed_writes = 0;
    }
    if !state.uncertain && state.failed_writes < 2 && state.last != Some(pressure) {
        match emit(pressure) {
            NoticeWrite::Complete => {
                state.last = Some(pressure);
                state.failed_writes = 0;
            }
            NoticeWrite::Uncertain => state.uncertain = true,
            NoticeWrite::NotWritten => state.failed_writes += 1,
        }
    }
}

pub(super) fn queued_notice(id: &JobId, pressure: bool) -> NoticeWrite {
    let explanation = if pressure {
        "Waiting for memory.\nStarts automatically when memory permits; other queue rules still apply."
    } else {
        "Waiting to start."
    };
    let message = format!("QUEUED\n{explanation}\nReference: {}\n", id.as_str());
    write_notice(&message)
}
