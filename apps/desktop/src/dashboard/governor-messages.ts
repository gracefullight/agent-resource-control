export const governorMessages = {
  label: "Agent Resource Control",
  checking: "Checking Governor…",
  active: "Active",
  paused: "Paused",
  unavailable: "Governor unavailable",
  applyingPause: "Applying pause…",
  applyingResume: "Applying resume…",
  uncertain: "Change not confirmed. Checking Governor…",
  pauseRejected:
    "Could not pause Governor. The prior confirmed setting is retained.",
  resumeRejected:
    "Could not resume Governor. The prior confirmed setting is retained.",
  rejectionReasons: {
    REVISION_EXHAUSTED: "The switch revision limit was reached.",
    RECEIPT_CAPACITY: "The mutation receipt limit was reached.",
    before_dispatch: "The request was rejected before dispatch.",
  },
  conflict:
    "Another client changed Governor. The current confirmed setting is shown.",
  unknownOutcome:
    "Current setting confirmed. The earlier change’s outcome is unknown.",
  activeHelp:
    "Pause scheduling to let supported waiting work proceed. Running work and usage measurement continue.",
  pausedHelp:
    "Scheduling paused. Supported waiting jobs can proceed without resubmitting. Running jobs and usage measurement continue.",
  resumeHelp:
    "Resuming restores admission rules; waiting work starts when those rules permit.",
  lastConfirmed: "Last confirmed setting",
  lastCheck: "Governor checked",
  diagnostics: "Governor diagnostics",
  revision: "Switch revision",
};
