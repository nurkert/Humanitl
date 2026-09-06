/// What `Doctor()` answered: one line per precondition of this machine,
/// mirror of `DoctorReport` and `DoctorCheck` (HUM-075, HUM-044).
///
/// The judgement is the daemon's and nobody else's. `humanitl_sandbox::doctor`
/// reads the machine into facts and decides over them; this file carries the
/// answer, and the setup screen shows it. Nothing here decides whether a check
/// passed -- an application that judged a second time would be a second
/// implementation of the same eleven questions (ADR-018).
///
/// # Three states on the wire, four things to say
///
/// `CheckStatus` has `OK`, `WARN` and `FAIL` and nothing else, and a fourth
/// value would be a change to the contract. A check that could not be
/// performed is therefore a warning with its own code, and a check nobody
/// asked for is a warning with another one. [DoctorCheck.measurement] gives
/// those two their own name again, so the screen can show them as what they
/// are: not a measurement, and never a passed one (CONVENTIONS 4.13).
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'diagnostic.dart';
import 'diagnostic_codes.dart';

part 'doctor.freezed.dart';

/// How a check went, mirror of `CheckStatus`.
///
/// The order is the ranking: [DoctorStatus.fail] is the worst, and
/// [DoctorReport.worst] takes the maximum by index.
enum DoctorStatus {
  /// A value this build does not know. Never folded into [DoctorStatus.ok]:
  /// a newer daemon may say something this app cannot read, and guessing
  /// "fine" is the one answer that must not be guessed.
  unknown,

  /// Measured and in order.
  ok,

  /// It runs, but not the way it should -- or it was not measured at all.
  warn,

  /// Without this nothing starts.
  fail,
}

/// Whether a line is a measurement, and if not, why not.
///
/// Both non-measurements arrive as [DoctorStatus.warn] because the wire has
/// room for nothing else. They are told apart by the code the daemon put on
/// them, not by the wording of the evidence: the code is the contract, the
/// sentence is for a person.
enum DoctorMeasurement {
  /// The daemon looked and can say what it found.
  measured,

  /// The check could not be performed on this machine: the source is missing
  /// or unreadable (`DOCTOR_012`). Nobody looked, so nothing is known.
  notPerformed,

  /// Nothing was contacted because nobody asked (`DOCTOR_013`). The daemon
  /// opens no connection as a side effect of a screen being opened; the
  /// fix names the command that measures it.
  notRequested,
}

/// One line of the doctor's report, mirror of `DoctorCheck`.
@freezed
abstract class DoctorCheck with _$DoctorCheck {
  /// Creates a line.
  const factory DoctorCheck({
    required String id,
    required DoctorStatus status,
    @Default('') String evidence,
    Diagnostic? diagnostic,
  }) = _DoctorCheck;

  const DoctorCheck._();

  /// Whether this line is a measurement, and if not, which kind of gap it is.
  DoctorMeasurement get measurement => switch (diagnostic?.code) {
    DiagnosticCodes.doctorNotPerformed => DoctorMeasurement.notPerformed,
    DiagnosticCodes.doctorNotContacted => DoctorMeasurement.notRequested,
    _ => DoctorMeasurement.measured,
  };

  /// True when nothing was measured, for either of the two reasons.
  bool get isMeasured => measurement == DoctorMeasurement.measured;

  /// True when this line alone forbids a start.
  bool get isFailure => status == DoctorStatus.fail;
}

/// The whole report, in display order, mirror of `DoctorReport`.
@freezed
abstract class DoctorReport with _$DoctorReport {
  /// Creates a report.
  const factory DoctorReport({
    @Default(<DoctorCheck>[]) List<DoctorCheck> checks,
  }) = _DoctorReport;

  const DoctorReport._();

  /// A report with no line at all. Not the same as a green one: it says that
  /// nothing was asked, and every reader has to treat it as such.
  static const DoctorReport empty = DoctorReport();

  /// The worst status in the report; [DoctorStatus.ok] for an empty one.
  DoctorStatus get worst {
    DoctorStatus worst = DoctorStatus.ok;
    for (final DoctorCheck check in checks) {
      if (check.status.index > worst.index) {
        worst = check.status;
      }
    }
    return worst;
  }

  /// True when at least one check failed.
  ///
  /// This is the one question the start button hangs on, and it is answered
  /// with the daemon's own verdict rather than recomputed here
  /// (`humanitl_sandbox::doctor::DoctorReport::has_failure`).
  bool get hasFailure =>
      checks.any((DoctorCheck check) => check.status == DoctorStatus.fail);

  /// The lines that were not measured, in display order.
  ///
  /// They are neither green nor red, and the screen names them separately:
  /// a machine where five checks could not run is a different machine from
  /// one where five passed.
  List<DoctorCheck> get unmeasured =>
      checks.where((DoctorCheck check) => !check.isMeasured).toList();

  /// The line with [id], or null when the report carries none.
  DoctorCheck? operator [](String id) {
    for (final DoctorCheck check in checks) {
      if (check.id == id) {
        return check;
      }
    }
    return null;
  }
}

/// The identifiers of the eleven checks, in the order the daemon reports them
/// (`humanitl_sandbox::doctor::CheckId::ALL`).
///
/// The application never builds this list to drive the screen -- the report
/// carries its own order, and a daemon that grows a twelfth line must show it.
/// The names are here so that the two lines with a row of their own can be
/// found, and so that a heading can be looked up per line.
abstract final class DoctorCheckId {
  /// Is `bwrap` there, and is it new enough?
  static const String bwrap = 'bwrap';

  /// May this user open a namespace?
  static const String userns = 'userns';

  /// Does the kernel know seccomp?
  static const String seccomp = 'seccomp';

  /// Is there a private runtime directory?
  static const String runtimeDir = 'runtime_dir';

  /// Is a systemd user session running?
  static const String systemdUser = 'systemd_user';

  /// Does the daemon answer, and does it speak the same contract?
  static const String daemon = 'daemon';

  /// Is the agent's command on the host?
  static const String agent = 'agent';

  /// Does the language model answer? Only when somebody asked.
  static const String llm = 'llm';

  /// Has the desktop a place for the tray icon?
  static const String tray = 'tray';

  /// Do renderer and graphics driver get along?
  static const String renderer = 'renderer';

  /// Is there room in the data directory for the recording?
  static const String diskSpace = 'disk_space';

  /// The two lines the setup screen shows in a row of their own, so the
  /// machine section does not repeat them.
  static const List<String> ownRow = <String>[daemon, llm];
}
