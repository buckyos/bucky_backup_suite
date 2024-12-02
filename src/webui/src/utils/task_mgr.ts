import buckyos from 'buckyos';

export class BackupTaskManager {
    private rpc_client: any;

    constructor() {
        // Initialize RPC client for backup control service
        this.rpc_client = new buckyos.kRPCClient("/kapi/backup_control");
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
        return result.plan_id;
    }

    async listBackupPlans() {
        const result = await this.rpc_client.call("list_backup_plan", {});
        return result.backup_plans;
    }

    async getBackupPlan(planId: string) {
        const result = await this.rpc_client.call("get_backup_plan", {
            plan_id: planId
        });
        return result;
    }

    async createBackupTask(planId: string, parentCheckpointId?: string) {
        const params: any = { plan_id: planId };
        if (parentCheckpointId) {
            params.parent_checkpoint_id = parentCheckpointId;
        }
        const result = await this.rpc_client.call("create_backup_task", params);
        return result;
    }

    async listBackupTasks(filter: "" | "running" = "") {
        const result = await this.rpc_client.call("list_backup_task", {
            filter: filter
        });
        return result.task_list;
    }

    async getTaskInfo(taskId: string) {
        const result = await this.rpc_client.call("get_task_info", {
            task_id: taskId
        });
        return result;
    }

    async resumeBackupTask(taskId: string) {
        const result = await this.rpc_client.call("resume_backup_task", {
            task_id: taskId
        });
        return result.result === "success";
    }

    async pauseBackupTask(taskId: string) {
        const result = await this.rpc_client.call("pause_backup_task", {
            task_id: taskId
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
}

// Export a singleton instance
export const taskManager = new BackupTaskManager();
