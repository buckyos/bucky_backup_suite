import { taskManager } from '@/utils/task_mgr';
import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { BSTaskPanel } from './bs_task_panel';


@customElement('bs-tasklist')
export class BSTaskList extends LitElement {
    filter: string;
    private task_timer;
    private current_task_list:Map<string, BSTaskPanel> = new Map();

    constructor() {
        super();
        this.filter = "all";
        this.task_timer = null;
        this.current_task_list = new Map();
        
    }

    setTaskFilter(filter: string) {
        this.filter = filter;
    }

    async relaod_tasklist() {
        let task_list = await taskManager.listBackupTasks(this.filter);
        this.current_task_list.clear();
        //clean all task panel
        let container = this.shadowRoot?.querySelector('.task-list-container');
        if(container) {
            container.innerHTML = '';
        }
        for (let taskid of task_list) {
            let task_info = await taskManager.getTaskInfo(taskid);
            let task_panel = document.createElement('bs-task-panel') as BSTaskPanel;
            task_panel.updateTaskInfo(task_info);
            this.current_task_list.set(taskid, task_panel);
            container?.appendChild(task_panel);
        }
    }

    firstUpdated() {
        this.relaod_tasklist();
        //创建Timer刷新task的状态
        this.task_timer = setInterval(async () => {
            //遍历current_task_list
            for (const [task_id, task_panel] of this.current_task_list) {
                if(task_panel.last_update_task_info) {
                    if (task_panel.last_update_task_info.state == "RUNNING") {
                        let task_info = await taskManager.getTaskInfo(task_id);
                        console.log("refresh task_info:", task_info);
                        task_panel.updateTaskInfo(task_info);
                    }
                }
            }
        }, 1000);
        taskManager.addTaskEventListener(async (event, data) => {
            if(event == "resume_task" || event == "pause_task" || event == "create_task") {
                await this.relaod_tasklist();
                console.log("task list reloaded");
            }
        });
    }

    disconnectedCallback() {
        super.disconnectedCallback();
        if (this.task_timer) {
            clearInterval(this.task_timer);
            this.task_timer = null;
        }
    }

    render() {
        return html`<div>
            <div class="task-list-container">
                <bs-task-panel title="Test Task" eta="3 hours" progress="58"></bs-task-panel>
            </div>
        `;
    }
  }