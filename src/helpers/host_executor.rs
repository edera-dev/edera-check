use nix::sched::{CloneFlags, setns, unshare};
use std::fs::File;
use std::os::unix::io::AsFd;
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct HostNamespaceExecutor {
    host_rt: Option<Arc<Runtime>>,
}

impl Drop for HostNamespaceExecutor {
    fn drop(&mut self) {
        if self.host_rt.is_some()
            && Arc::strong_count(self.host_rt.as_ref().expect("just checked")) <= 1
            && let Some(rt) = self.host_rt.take()
        {
            Arc::into_inner(rt).unwrap().shutdown_background();
        }
    }
}

impl HostNamespaceExecutor {
    /// Creates a new tokio runtime running with the net/mount/pid/ipc/etc namespaces
    /// of the target pid. Note that if this is run from a pid namespace (e.g. a container without
    /// `--pid=host`/`hostPid: true` or equivalent) the PID provided here
    pub fn new() -> Self {
        if !Self::in_host_pid_namespace() {
            panic!("Cannot create host namespace executor, not running in host pid namespace");
        }
        Self::runtime_in_target_process_namespaces(1)
    }

    /// Run a future in the host context. Note that the closure will *not* run in
    /// in the pid namespace, because you have to fork/execve (run a new process)
    /// for that to take effect. If you really need the host's pid namespace specifically,
    /// use `Command:new()` to launch a new subprocess from the future.
    pub fn spawn_in_host_ns<F>(&self, f: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.host_rt
            .as_ref()
            .expect("host executor dropped")
            .spawn(f)
    }

    fn runtime_in_target_process_namespaces(target_pid: u32) -> Self {
        let rt = std::thread::spawn(move || {
            unshare(CloneFlags::CLONE_FS).expect("Failed to unshare FS attributes");
            // Enter namespaces
            for (ns, flag) in &[
                ("mnt", CloneFlags::CLONE_NEWNS),
                ("net", CloneFlags::CLONE_NEWNET),
                ("uts", CloneFlags::CLONE_NEWUTS),
                ("ipc", CloneFlags::CLONE_NEWIPC),
                ("pid", CloneFlags::CLONE_NEWPID),
            ] {
                let f = File::open(format!("/proc/{}/ns/{}", target_pid, ns))
                    .expect("could not open pid {target_pid} ns file {ns}");
                setns(f.as_fd(), *flag)
                    .unwrap_or_else(|_| panic!("could not setns for pid {target_pid} ns {ns}"));
            }

            Builder::new_multi_thread()
                .thread_name(format!("pid-{target_pid}-executor-threadset"))
                .build()
                .expect("could not spawn pid-{target_pid} threadset")
        })
        .join()
        .expect("could not create pid-{target_pid} executor");

        Self {
            host_rt: Some(Arc::new(rt)),
        }
    }

    /// A truly dirty hack to figure out if we're running in the host's pid namespace or not.
    /// Checks to see if pid 2 is `kthreadd` - if it isn't, we're definitely in a pid namespace (container).
    /// If it is, we're reasonably sure we're in the top-level pid namespace.
    /// Q: Is this reliable?
    /// A: I have *not yet* seen an environment where it *does not* work, and if you find one I'll send you five bucks.
    fn in_host_pid_namespace() -> bool {
        if !std::path::Path::new("/proc/2").exists() {
            return false; // Definitely in a namespace, no pid 2
        }

        let comm = std::fs::read_to_string("/proc/2/comm").expect("can read proc");
        comm.trim() == "kthreadd"
    }
}
