import buckyos from 'buckyos';

export interface TaskInfo {
    taskid: string;
    task_type: string;
    owner_plan_id: string;
    checkpoint_id: string;
    total_size: number;
    completed_size: number;
    state: 'RUNNING' | 'PENDING' | 'PAUSED' | 'DONE' | 'FAILED';
    create_time: number;//unix timestamp 
    update_time: number;//unix timestamp 
    item_count: number;
    completed_item_count: number;
    wait_transfer_item_count: number;
    last_log_content: string | null;
}

export interface BackupPlanInfo {
    plan_id: string;
    title: string;
    description: string;
    type_str: string;
    last_checkpoint_index: number;
    source: string;
    target: string;
}

export class BackupTaskManager {
    private rpc_client: any;
    //可以关注task事件(全部task)
    private task_event_listeners: ((event: string, data: any) => void | Promise<void>)[] = [];

    constructor() {
        // Initialize RPC client for backup control service
        this.rpc_client = new buckyos.kRPCClient("/kapi/backup_control");
        this.task_event_listeners = [];
    }

    addTaskEventListener(listener: (event: string, data: any) => void | Promise<void>) {
        this.task_event_listeners.push(listener);
    }

    async emitTaskEvent(event: string, data: any) {
        // 使用 Promise.all 等待所有监听器执行完成
        await Promise.all(
            this.task_event_listeners.map(listener => {
                try {
                    return Promise.resolve(listener(event, data));
                } catch (error) {
                    console.error('Error in task event listener:', error);
                    return Promise.resolve();
                }
            })
            
        );
    }

    async createBackupPlan(params: {
        type_str: string,
        source_type: string,
        source: string,
        target_type: string,
        target: string,
        title: string,
        description: string
    }): Promise<string> {
        const result = await this.rpc_client.call("create_backup_plan", params);
        result.type_str = params.type_str;
        result.source = params.source;
        result.target = params.target;
        result.title = params.title;
        result.description = params.description;
        await this.emitTaskEvent("create_plan", result);
        return result.plan_id;
    }

    async listBackupPlans() {
        const result = await this.rpc_client.call("list_backup_plan", {});
        return result.backup_plans;
    }

    async getBackupPlan(planId: string): Promise<BackupPlanInfo> {
        const result = await this.rpc_client.call("get_backup_plan", {
            plan_id: planId
        });
        result.plan_id = planId;
        return result;
    }

    async createBackupTask(planId: string, parentCheckpointId: string | null) {
        const params: any = { plan_id: planId };
        if (parentCheckpointId) {
            params.parent_checkpoint_id = parentCheckpointId;
        }
        const result = await this.rpc_client.call("create_backup_task", params);
        return result;
    }

    async createRestoreTask(planId: string, checkpointId: string, targetLocationUrl: string, is_clean_folder?: boolean) {
        const params: any = { plan_id: planId, checkpoint_id: checkpointId, cfg: {
            restore_location_url: targetLocationUrl,
            is_clean_restore: is_clean_folder
        } };

        const result = await this.rpc_client.call("create_restore_task", params);
        return result;
    }

    async listBackupTasks(filter: "all" | "running" | "paused" = "all") {
        const result = await this.rpc_client.call("list_backup_task", {
            filter: filter
        });
        return result.task_list;
    }

    async getTaskInfo(taskId: string): Promise<TaskInfo> {
        const result = await this.rpc_client.call("get_task_info", {
            taskid: taskId
        });
        return result;
    }

    async resumeBackupTask(taskId: string) {
        const result = await this.rpc_client.call("resume_backup_task", {
            taskid: taskId
        });
        await this.emitTaskEvent("resume_task", taskId);
        return result.result === "success";
    }

    async pauseBackupTask(taskId: string) {
        const result = await this.rpc_client.call("pause_backup_task", {
            taskid: taskId
        });
        return result.result === "success";
    }

    async validatePath(path: string) {
        const result = await this.rpc_client.call("validate_path", {
            path: path
        });
        console.log(result);
        return result.path_exist;
    }

    async resume_last_working_task() {
        let taskid_list = await this.listBackupTasks("paused");  
        if(taskid_list.length > 0) {
            let last_task = taskid_list[0];
            console.log("resume last task:", last_task);
            this.resumeBackupTask(last_task);
            await this.emitTaskEvent("resume_task", last_task);
        }
    }

    async pause_all_tasks() {
        let taskid_list = await this.listBackupTasks("running");
        for(let taskid of taskid_list) {
            this.pauseBackupTask(taskid);
            await this.emitTaskEvent("pause_task", taskid);
        }
    }

}

// Export a singleton instance
export const taskManager = new BackupTaskManager();
