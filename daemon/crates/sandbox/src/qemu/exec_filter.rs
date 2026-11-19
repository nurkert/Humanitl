//! Der seccomp-Filter, den bubblewrap jedem Helfer der Kernel-Stufe vor
//! dessen Start auflegt (HUM-557, `--seccomp`).
//!
//! **Wozu.** Die Unit führt nur die Dateien ihrer Liste aus (HUM-480,
//! [`super::programs`]), und seit HUM-557 gibt es in keinem Gefängnis eine
//! beschreibbare Einhängung ohne `noexec` mehr. Ein Weg blieb: ein `memfd`.
//! Er liegt auf einer Einhängung des Kernels außerhalb jedes Namensraums,
//! `vm.memfd_noexec` ist auf den gemessenen Rechnern 0, und kein Prozess der
//! Unit darf es heben. Ein übernommener Helfer konnte also ein Programm
//! eigener Wahl in einen `memfd` schreiben und mit `fexecve` starten
//! (gemessen in HUM-480 mit einem statischen Programm in einer Test-Unit).
//!
//! **Was der Filter verweigert** (mit `EPERM`, alles andere läuft):
//! - `memfd_create` ohne `MFD_NOEXEC_SEAL` ([`ExecFilter::Helper`]). Ein
//!   `memfd` mit dieser Marke ist nie ausführbar, auch nicht nach `fchmod`;
//!   ohne sie wäre er es bei `vm.memfd_noexec = 0`, auch ohne `MFD_EXEC`.
//!   Deshalb prüft der Filter das Fehlen der einen Marke, nicht das
//!   Vorhandensein der anderen. Kein Helfer außer dem Emulator legt einen
//!   `memfd` an: Den Speicher des Gasts bekommen virtiofsd und das
//!   vsock-Gerät über vhost-user als Deskriptor.
//! - `execveat` in jeder Form. Kein Helfer ruft es nach seinem Start auf, und
//!   bubblewrap startet den Helfer mit `execvp`, also `execve`. Das schließt
//!   `fexecve` (glibc ruft `execveat` mit leerem Pfad und `AT_EMPTY_PATH`)
//!   und jeden anderen Start über einen Deskriptor.
//! - Jeden Aufruf der x32-ABI: Sie teilt die Architektur mit `x86_64`, nur
//!   ihre Nummern tragen das Bit [`X32_SYSCALL_BIT`], und ein Filter, der nur
//!   Nummern vergleicht, ließe `memfd_create` unter seiner x32-Nummer durch.
//!   Eine fremde Architektur beendet den Prozess.
//!
//! `execve` bleibt erlaubt: bubblewrap braucht es für den Start des Helfers,
//! nach dem Filter, und ein Filter ohne Zustand kann den ersten Aufruf nicht
//! von einem späteren unterscheiden. `execve` über `/proc/self/fd/<n>` auf
//! einen `memfd` bleibt trotzdem zu, weil kein ausführbarer `memfd` entsteht.
//!
//! **Der Emulator** ([`ExecFilter::Emulator`]) legt den Speicher des Gasts
//! selbst als `memfd` an (`memory-backend-memfd`), ohne `MFD_NOEXEC_SEAL`;
//! die Regel für `memfd_create` bekäme er deshalb nicht. Ihn hält
//! `-sandbox spawn=deny` von `execve` fern, und dieser Filter von
//! `execveat`; beides zusammen schließt den Start eines `memfd`.
//!
//! Der Filter ist ein Programm in klassischem BPF, wie es bubblewrap aus dem
//! Deskriptor liest: Folgen von acht Bytes (`struct sock_filter`) in der
//! Bytefolge des Rechners. Es ist so kurz, dass ein Interpreter im Test jede
//! Antwort nachrechnet; die KVM-Tests prüfen es in echten Gefängnissen.

/// Der Deskriptor, auf dem das Skript der Unit bubblewrap den Filter gibt
/// (`--seccomp`); 3 bis 6 sind die Deskriptoren der Übergabe
/// ([`super::lines::ROOT_SOCKET_FD`]).
pub(crate) const FILTER_FD: i32 = 7;

/// `AUDIT_ARCH_X86_64`.
const AUDIT_ARCH_X86_64: u32 = 0xC000_003E;
/// Das Bit der x32-ABI in der Nummer eines Aufrufs (`__X32_SYSCALL_BIT`).
pub(crate) const X32_SYSCALL_BIT: u32 = 0x4000_0000;
/// `__NR_memfd_create` auf `x86_64`.
pub(crate) const SYS_MEMFD_CREATE: u32 = 319;
/// `__NR_execveat` auf `x86_64`.
pub(crate) const SYS_EXECVEAT: u32 = 322;
/// `MFD_NOEXEC_SEAL` (Linux 6.3).
pub(crate) const MFD_NOEXEC_SEAL: u32 = 0x0008;

/// Die Antworten des Filters.
const RET_ALLOW: u32 = 0x7fff_0000;
const RET_KILL_PROCESS: u32 = 0x8000_0000;
/// `SECCOMP_RET_ERRNO | EPERM`.
pub(crate) const RET_EPERM: u32 = 0x0005_0001;

/// Die Befehle, die der Filter braucht.
const LD_W_ABS: u16 = 0x20;
const JEQ_K: u16 = 0x15;
const JGE_K: u16 = 0x35;
const JSET_K: u16 = 0x45;
const RET_K: u16 = 0x06;

/// Wo `struct seccomp_data` die Nummer, die Architektur und die unteren 32
/// Bit des zweiten Arguments trägt (little-endian).
const NR: u32 = 0;
const ARCH: u32 = 4;
const ARG1_LOW: u32 = 16 + 8;

/// Welcher Filter ein Gefängnis bekommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecFilter {
    /// virtiofsd und das vsock-Gerät: kein `execveat`, kein ausführbarer
    /// `memfd`.
    Helper,
    /// Der Emulator: kein `execveat`; `execve` verbietet er sich selbst.
    Emulator,
}

/// Ein Befehl in klassischem BPF (`struct sock_filter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Insn {
    pub(crate) code: u16,
    pub(crate) jt: u8,
    pub(crate) jf: u8,
    pub(crate) k: u32,
}

/// Wohin ein Sprung geht, bevor die Abstände feststehen.
#[derive(Debug, Clone, Copy)]
enum To {
    Next,
    Allow,
    Deny,
    Kill,
}

/// Ein Schritt des Filters vor dem Auflösen der Sprünge.
#[derive(Debug, Clone, Copy)]
enum Step {
    Load(u32),
    Jump { code: u16, k: u32, yes: To, no: To },
}

impl ExecFilter {
    /// Der Name der Datei im Laufzeitverzeichnis (`files/`), aus der das
    /// Skript den Filter liest.
    pub(crate) const fn file_name(self) -> &'static str {
        match self {
            Self::Helper => "exec-filter-helper.bpf",
            Self::Emulator => "exec-filter-emulator.bpf",
        }
    }

    fn steps(self) -> Vec<Step> {
        let mut steps = vec![
            Step::Load(ARCH),
            Step::Jump {
                code: JEQ_K,
                k: AUDIT_ARCH_X86_64,
                yes: To::Next,
                no: To::Kill,
            },
            Step::Load(NR),
            Step::Jump {
                code: JGE_K,
                k: X32_SYSCALL_BIT,
                yes: To::Deny,
                no: To::Next,
            },
            Step::Jump {
                code: JEQ_K,
                k: SYS_EXECVEAT,
                yes: To::Deny,
                no: To::Next,
            },
        ];
        if self == Self::Helper {
            steps.extend([
                Step::Jump {
                    code: JEQ_K,
                    k: SYS_MEMFD_CREATE,
                    yes: To::Next,
                    no: To::Allow,
                },
                Step::Load(ARG1_LOW),
                Step::Jump {
                    code: JSET_K,
                    k: MFD_NOEXEC_SEAL,
                    yes: To::Allow,
                    no: To::Deny,
                },
            ]);
        }
        steps
    }

    /// Das Programm: die Schritte, dann `Allow`, `Deny` und `Kill`.
    pub(crate) fn program(self) -> Vec<Insn> {
        let steps = self.steps();
        let allow = steps.len();
        let target = |to: To, at: usize| -> u8 {
            let index = match to {
                To::Next => at + 1,
                To::Allow => allow,
                To::Deny => allow + 1,
                To::Kill => allow + 2,
            };
            // Höchstens ein Dutzend Befehle: Jeder Abstand passt in ein Byte.
            u8::try_from(index - at - 1).unwrap_or(u8::MAX)
        };
        let mut program: Vec<Insn> = steps
            .iter()
            .enumerate()
            .map(|(at, step)| match *step {
                Step::Load(offset) => Insn {
                    code: LD_W_ABS,
                    jt: 0,
                    jf: 0,
                    k: offset,
                },
                Step::Jump { code, k, yes, no } => Insn {
                    code,
                    jt: target(yes, at),
                    jf: target(no, at),
                    k,
                },
            })
            .collect();
        for k in [RET_ALLOW, RET_EPERM, RET_KILL_PROCESS] {
            program.push(Insn {
                code: RET_K,
                jt: 0,
                jf: 0,
                k,
            });
        }
        program
    }

    /// Das Programm, wie bubblewrap es aus dem Deskriptor liest.
    pub(crate) fn bytes(self) -> Vec<u8> {
        self.program()
            .iter()
            .flat_map(|insn| {
                let mut bytes = Vec::with_capacity(8);
                bytes.extend_from_slice(&insn.code.to_ne_bytes());
                bytes.push(insn.jt);
                bytes.push(insn.jf);
                bytes.extend_from_slice(&insn.k.to_ne_bytes());
                bytes
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const AUDIT_ARCH_I386: u32 = 0x4000_0003;
    const SYS_EXECVE: u32 = 59;
    const SYS_OPENAT: u32 = 257;
    const MFD_CLOEXEC: u32 = 0x0001;
    const MFD_ALLOW_SEALING: u32 = 0x0002;
    const MFD_EXEC: u32 = 0x0010;

    /// Ein Interpreter für genau die Befehle des Filters, über
    /// `struct seccomp_data` (Nummer, Architektur, Argumente).
    fn run(program: &[Insn], nr: u32, arch: u32, args: [u64; 6]) -> u32 {
        let word = |offset: u32| -> u32 {
            match offset {
                0 => nr,
                4 => arch,
                16..=63 if offset.is_multiple_of(4) => {
                    let arg = args[((offset - 16) / 8) as usize];
                    if (offset - 16).is_multiple_of(8) {
                        (arg & 0xffff_ffff) as u32
                    } else {
                        (arg >> 32) as u32
                    }
                }
                _ => panic!("the filter reads offset {offset}"),
            }
        };
        let mut acc = 0u32;
        let mut pc = 0usize;
        loop {
            let insn = program[pc];
            let jump = |taken: bool| pc + 1 + usize::from(if taken { insn.jt } else { insn.jf });
            pc = match insn.code {
                LD_W_ABS => {
                    acc = word(insn.k);
                    pc + 1
                }
                JEQ_K => jump(acc == insn.k),
                JGE_K => jump(acc >= insn.k),
                JSET_K => jump(acc & insn.k != 0),
                RET_K => return insn.k,
                other => panic!("unknown instruction {other:#x}"),
            };
        }
    }

    fn call(filter: ExecFilter, nr: u32, arg1: u64) -> u32 {
        run(
            &filter.program(),
            nr,
            AUDIT_ARCH_X86_64,
            [0, arg1, 0, 0, 0, 0],
        )
    }

    /// Ein Helfer bekommt keinen ausführbaren `memfd` und kein `execveat`;
    /// alles andere, auch `execve` und ein versiegelter `memfd`, läuft.
    ///
    /// Mutationsbeweis: Prüft der Filter `MFD_EXEC` statt des Fehlens von
    /// `MFD_NOEXEC_SEAL`, käme `memfd_create(name, 0)` durch, und der Test
    /// wird rot; ohne den Sprung für `execveat` ebenso.
    #[test]
    fn a_helper_gets_no_executable_memfd_and_no_execveat() {
        let helper = ExecFilter::Helper;
        for flags in [
            0,
            u64::from(MFD_CLOEXEC),
            u64::from(MFD_CLOEXEC | MFD_ALLOW_SEALING),
            u64::from(MFD_EXEC),
            // Die oberen 32 Bit liest der Kernel für ein `unsigned int`
            // nicht; eine Marke dort zählt nicht.
            u64::from(MFD_NOEXEC_SEAL) << 32,
        ] {
            assert_eq!(
                call(helper, SYS_MEMFD_CREATE, flags),
                RET_EPERM,
                "{flags:#x}"
            );
        }
        for flags in [MFD_NOEXEC_SEAL, MFD_NOEXEC_SEAL | MFD_CLOEXEC] {
            assert_eq!(
                call(helper, SYS_MEMFD_CREATE, u64::from(flags)),
                RET_ALLOW,
                "{flags:#x}"
            );
        }
        assert_eq!(call(helper, SYS_EXECVEAT, 0), RET_EPERM);
        assert_eq!(call(helper, SYS_EXECVE, 0), RET_ALLOW);
        assert_eq!(call(helper, SYS_OPENAT, 0), RET_ALLOW);
    }

    /// Der Emulator legt seinen Speicher als `memfd` an; er bekommt nur kein
    /// `execveat`.
    #[test]
    fn the_emulator_keeps_its_memfd_but_gets_no_execveat() {
        let emulator = ExecFilter::Emulator;
        assert_eq!(
            call(
                emulator,
                SYS_MEMFD_CREATE,
                u64::from(MFD_CLOEXEC | MFD_ALLOW_SEALING)
            ),
            RET_ALLOW
        );
        assert_eq!(call(emulator, SYS_EXECVEAT, 0), RET_EPERM);
        assert_eq!(call(emulator, SYS_EXECVE, 0), RET_ALLOW);
    }

    /// Die x32-ABI bekommt `EPERM`, eine fremde Architektur endet.
    #[test]
    fn x32_and_foreign_architectures_do_not_pass() {
        for filter in [ExecFilter::Helper, ExecFilter::Emulator] {
            let program = filter.program();
            for nr in [SYS_MEMFD_CREATE, SYS_EXECVEAT, SYS_OPENAT] {
                assert_eq!(
                    run(&program, X32_SYSCALL_BIT | nr, AUDIT_ARCH_X86_64, [0; 6]),
                    RET_EPERM
                );
                assert_eq!(run(&program, nr, AUDIT_ARCH_I386, [0; 6]), RET_KILL_PROCESS);
            }
        }
    }

    /// bubblewrap liest Folgen von acht Bytes; die Datei ist genau das
    /// Programm.
    #[test]
    fn the_file_is_the_program_in_eight_byte_steps() {
        for filter in [ExecFilter::Helper, ExecFilter::Emulator] {
            let program = filter.program();
            let bytes = filter.bytes();
            assert_eq!(bytes.len(), program.len() * 8);
            let first = &bytes[..8];
            assert_eq!(u16::from_ne_bytes([first[0], first[1]]), LD_W_ABS);
            assert_eq!(
                u32::from_ne_bytes([first[4], first[5], first[6], first[7]]),
                ARCH
            );
        }
        assert_ne!(
            ExecFilter::Helper.file_name(),
            ExecFilter::Emulator.file_name()
        );
    }
}
