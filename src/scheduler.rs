use crate::types::{Task, TaskInfo, TaskStatus, SchedulerInfo};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

#[derive(Clone)]
pub struct Scheduler {
    tasks: Arc<DashMap<String, TaskInfo>>,
    semaphore: Arc<Semaphore>,
    task_sender: mpsc::UnboundedSender<TaskInfo>,
}

impl Scheduler {
    pub fn new(max_workers: usize) -> Self {
        let (task_sender, task_receiver) = mpsc::unbounded_channel::<TaskInfo>();
        let tasks = Arc::new(DashMap::new());
        let semaphore = Arc::new(Semaphore::new(max_workers));
        
        let scheduler = Scheduler {
            tasks: tasks.clone(),
            semaphore: semaphore.clone(),
            task_sender,
        };
        
        // Start the worker loop
        tokio::spawn(Self::worker_loop(
            task_receiver,
            tasks,
            semaphore,
        ));
        
        scheduler
    }
    
    pub fn schedule_task(&self, task: Task) -> crate::Result<String> {
        let task_id = Uuid::new_v4().to_string();
        let task_info = TaskInfo {
            id: task_id.clone(),
            task,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            status: TaskStatus::Pending,
            error: None,
        };
        
        self.tasks.insert(task_id.clone(), task_info.clone());
        
        self.task_sender.send(task_info)
            .map_err(|e| crate::DoomsdayError::scheduler(format!("Failed to schedule task: {}", e)))?;
        
        Ok(task_id)
    }
    
    pub fn get_task(&self, task_id: &str) -> Option<TaskInfo> {
        self.tasks.get(task_id).map(|entry| entry.clone())
    }
    
    pub fn list_tasks(&self) -> Vec<TaskInfo> {
        self.tasks.iter().map(|entry| entry.clone()).collect()
    }
    
    pub fn get_info(&self) -> SchedulerInfo {
        let tasks: Vec<TaskInfo> = self.list_tasks();
        let pending_tasks = tasks.iter().filter(|t| matches!(t.status, TaskStatus::Pending)).count();
        let running_tasks = tasks.iter().filter(|t| matches!(t.status, TaskStatus::Running)).count();
        let available_permits = self.semaphore.available_permits();
        let total_workers = available_permits + running_tasks;
        
        SchedulerInfo {
            workers: total_workers,
            pending_tasks,
            running_tasks,
        }
    }
    
    async fn worker_loop(
        mut task_receiver: mpsc::UnboundedReceiver<TaskInfo>,
        tasks: Arc<DashMap<String, TaskInfo>>,
        semaphore: Arc<Semaphore>,
    ) {
        while let Some(mut task_info) = task_receiver.recv().await {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let tasks_clone = tasks.clone();
            
            tokio::spawn(async move {
                let _permit = permit; // Keep permit until task completes
                
                // Update task status to running
                task_info.status = TaskStatus::Running;
                task_info.started_at = Some(Utc::now());
                tasks_clone.insert(task_info.id.clone(), task_info.clone());
                
                // Execute the task
                let result = Self::execute_task(&task_info.task).await;
                
                // Update task status based on result
                task_info.completed_at = Some(Utc::now());
                match result {
                    Ok(()) => {
                        task_info.status = TaskStatus::Completed;
                    },
                    Err(e) => {
                        task_info.status = TaskStatus::Failed;
                        task_info.error = Some(e.to_string());
                    },
                }
                
                tasks_clone.insert(task_info.id.clone(), task_info);
            });
        }
    }
    
    async fn execute_task(task: &Task) -> crate::Result<()> {
        match task {
            Task::RefreshBackend { backend_name } => {
                tracing::info!("Refreshing backend: {}", backend_name);
                // TODO: Implement backend refresh logic
                sleep(Duration::from_millis(100)).await; // Placeholder
                Ok(())
            },
            Task::RenewAuthToken { backend_name } => {
                tracing::info!("Renewing auth token for backend: {}", backend_name);
                // TODO: Implement auth token renewal logic
                sleep(Duration::from_millis(50)).await; // Placeholder
                Ok(())
            },
        }
    }
    
    pub fn cleanup_completed_tasks(&self, max_age: Duration) {
        let cutoff = Utc::now() - chrono::Duration::from_std(max_age).unwrap_or_default();
        
        let expired_task_ids: Vec<String> = self.tasks
            .iter()
            .filter(|entry| {
                let task = entry.value();
                matches!(task.status, TaskStatus::Completed | TaskStatus::Failed) &&
                task.completed_at.map_or(false, |completed| completed < cutoff)
            })
            .map(|entry| entry.key().clone())
            .collect();
        
        for task_id in expired_task_ids {
            self.tasks.remove(&task_id);
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(4) // Default to 4 workers
    }
}