//! The agent in a guest kernel is no root (HUM-445).
//!
//! Under bwrap the agent runs with the caller's uid in a user namespace and
//! without any capability; bwrap drops them. In a guest kernel the shim is
//! PID 1 and root with every capability, and a child it forks inherits that.
//! Root in the guest would see its own kernel threads, open `/proc/1/mem` of
//! PID 1 for writing and so take over the process that holds the report and
//! the door. Before the agent's filter, the agent's child and every further
//! shell therefore give everything up, in this order:
//!
//! 1. every capability out of the bounding set (`PR_CAPBSET_DROP`, while
//!    `CAP_SETPCAP` is still there), the ambient set cleared;
//! 2. the supplementary groups emptied, then group and user set to the
//!    plan's, real, effective and saved alike; the kernel clears the
//!    permitted and effective sets with the change of the user;
//! 3. the inheritable set emptied with `capset`, and `PR_SET_NO_NEW_PRIVS`;
//! 4. the proof, read back from the kernel: no id is 0 or the old one, every
//!    capability set is empty, no bit of the bounding set is left.
//!
//! The ids are those of the user that started the daemon, the same that
//! bwrap maps into its sandbox without `--uid`, and the ones the guest's
//! `/etc/passwd` names. Everything here is a raw system call without
//! allocation: a further shell runs it between `fork` and `exec` of a
//! process that has other threads.

use libc::c_int;

/// The header of `capget(2)` and `capset(2)`.
#[repr(C)]
struct CapHeader {
    version: u32,
    pid: c_int,
}

/// One of the two halves of the 64 capability bits.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CapData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

/// `_LINUX_CAPABILITY_VERSION_3`: two halves.
const CAP_VERSION_3: u32 = 0x2008_0522;

/// Every capability number the kernel may know; numbers above its last one
/// answer `EINVAL`, which counts as gone.
const CAP_NUMBERS: c_int = 64;

/// Gives up root and every capability, then proves it.
///
/// # Errors
///
/// A short reason without spaces, the step that failed.
pub fn drop_to(uid: u32, gid: u32) -> Result<(), &'static str> {
    if uid == 0 || gid == 0 {
        return Err("identity:the-plan-names-root");
    }
    for cap in 0..CAP_NUMBERS {
        // SAFETY: prctl with PR_CAPBSET_DROP takes a capability number.
        let rc = unsafe { libc::prctl(libc::PR_CAPBSET_DROP, cap, 0, 0, 0) };
        if rc != 0 && errno() != libc::EINVAL {
            return Err("identity:capbset-drop");
        }
    }
    // SAFETY: prctl with PR_CAP_AMBIENT_CLEAR_ALL takes only constants.
    if unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    } != 0
    {
        return Err("identity:ambient-clear");
    }
    // SAFETY: setgroups with a count of 0 reads no pointer.
    if unsafe { libc::setgroups(0, std::ptr::null()) } != 0 {
        return Err("identity:setgroups");
    }
    // SAFETY: setresgid and setresuid take integers.
    if unsafe { libc::setresgid(gid, gid, gid) } != 0 {
        return Err("identity:setresgid");
    }
    // SAFETY: as above.
    if unsafe { libc::setresuid(uid, uid, uid) } != 0 {
        return Err("identity:setresuid");
    }
    let mut header = CapHeader {
        version: CAP_VERSION_3,
        pid: 0,
    };
    let empty = [CapData::default(); 2];
    // SAFETY: a valid header and two data halves that outlive the call.
    if unsafe { libc::syscall(libc::SYS_capset, &raw mut header, empty.as_ptr()) } != 0 {
        return Err("identity:capset");
    }
    // SAFETY: prctl with PR_SET_NO_NEW_PRIVS takes only constants.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err("identity:no-new-privs");
    }
    prove(uid, gid)
}

/// Arms `PR_SET_PDEATHSIG` again after [`drop_to`]: the kernel clears it
/// when the credentials change, so a child that gave up root would outlive
/// PID 1. Then checks, as before the drop, that the parent still lives.
///
/// # Errors
///
/// A short reason without spaces.
pub fn keep_parent_death(parent: libc::pid_t) -> Result<(), &'static str> {
    // SAFETY: prctl with PR_SET_PDEATHSIG only records a signal number.
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0 {
        return Err("identity:pdeathsig");
    }
    // SAFETY: getppid has no preconditions.
    if unsafe { libc::getppid() } != parent {
        return Err("identity:parent-gone");
    }
    Ok(())
}

/// Reads back what [`drop_to`] did.
fn prove(uid: u32, gid: u32) -> Result<(), &'static str> {
    let mut users: [libc::uid_t; 3] = [0; 3];
    let mut groups: [libc::gid_t; 3] = [0; 3];
    let [real_user, effective_user, saved_user] = &mut users;
    let [real_group, effective_group, saved_group] = &mut groups;
    // SAFETY: six pointers to integers on this stack.
    let ids_read = unsafe {
        libc::getresuid(real_user, effective_user, saved_user) == 0
            && libc::getresgid(real_group, effective_group, saved_group) == 0
    };
    if !ids_read || users != [uid; 3] || groups != [gid; 3] {
        return Err("identity:ids-not-the-plans");
    }
    let mut header = CapHeader {
        version: CAP_VERSION_3,
        pid: 0,
    };
    let mut data = [CapData::default(); 2];
    // SAFETY: a valid header and two data halves the kernel fills.
    if unsafe { libc::syscall(libc::SYS_capget, &raw mut header, data.as_mut_ptr()) } != 0 {
        return Err("identity:capget");
    }
    if data
        .iter()
        .any(|half| half.effective != 0 || half.permitted != 0 || half.inheritable != 0)
    {
        return Err("identity:capabilities-left");
    }
    for cap in 0..CAP_NUMBERS {
        // SAFETY: prctl with PR_CAPBSET_READ takes a capability number.
        if unsafe { libc::prctl(libc::PR_CAPBSET_READ, cap, 0, 0, 0) } == 1 {
            return Err("identity:bounding-set-left");
        }
    }
    // SAFETY: prctl with PR_CAP_AMBIENT_IS_SET takes a capability number.
    if (0..CAP_NUMBERS).any(|cap| unsafe {
        libc::prctl(libc::PR_CAP_AMBIENT, libc::PR_CAP_AMBIENT_IS_SET, cap, 0, 0) == 1
    }) {
        return Err("identity:ambient-left");
    }
    // SAFETY: prctl with PR_GET_NO_NEW_PRIVS takes only constants.
    if unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1 {
        return Err("identity:no-new-privs-not-set");
    }
    Ok(())
}

fn errno() -> c_int {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Root is never the target, whatever the plan says.
    #[test]
    fn root_is_refused_before_anything_changes() {
        assert_eq!(drop_to(0, 1000), Err("identity:the-plan-names-root"));
        assert_eq!(drop_to(1000, 0), Err("identity:the-plan-names-root"));
    }

    /// An unprivileged test process cannot drop the bounding set; the first
    /// step fails closed and names itself, nothing is half done.
    #[test]
    fn without_cap_setpcap_the_first_step_refuses() {
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        // Run in a child so that this test process keeps its state.
        // SAFETY: the child calls only drop_to and _exit.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            let code = match drop_to(4242, 4242) {
                Err("identity:capbset-drop") => 0,
                _ => 1,
            };
            // SAFETY: _exit takes an int.
            unsafe { libc::_exit(code) };
        }
        let mut status = 0;
        // SAFETY: waits for the child just forked.
        unsafe { libc::waitpid(pid, &raw mut status, 0) };
        assert!(libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0);
    }
}
