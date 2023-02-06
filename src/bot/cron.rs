use log::*;
use tokio::sync::mpsc;
use tokio_cron_scheduler::{Job, JobScheduler};
pub struct Cron {
    scheduler: JobScheduler,
}
impl Cron {
    pub async fn new() -> anyhow::Result<Self> {
        let sched = JobScheduler::new().await?;
        sched.start().await?;
        Ok(Self { scheduler: sched })
    }

    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        debug!("Shutting down cron scheduler");
        self.scheduler.shutdown().await?;
        Ok(())
    }

    pub async fn add_job(&self, cron_str: &str) -> anyhow::Result<mpsc::Receiver<()>> {
        let (tx, rx) = mpsc::channel(1);
        let job = Job::new_async(cron_str, move |uuid, _l| {
            let tx = tx.clone();
            Box::pin(async move {
                debug!("Running cron job: {:?}", uuid);
                if let Err(e) = tx.send(()).await {
                    error!("Cron job send error: {:?},uuid: {:?}", e, uuid);
                }
            })
        })?;
        self.scheduler.add(job).await?;
        Ok(rx)
    }
}
