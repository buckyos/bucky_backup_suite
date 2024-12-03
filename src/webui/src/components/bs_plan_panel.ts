import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import templateContent from './bs_plan_panel.template?raw';
import { taskManager, BackupPlanInfo } from '@/utils/task_mgr';

@customElement('bs-plan-panel')
export class BSPlanPanel extends LitElement {
    static properties = {
        plan_title: { type: String },
        type_str: { type: String },
        source: { type: String },
        target: { type: String },
        is_running: { type: Boolean },
        last_backup_time: { type: String },
        last_backup_size: { type: String },
    };

    plan_title: string;
    type_str: string;
    source: string;
    target: string;
    is_running: boolean;
    last_backup_time: string;
    last_backup_size: string;
    plan_id: string;

    template_compiled: HandlebarsTemplateDelegate<any>;

    constructor() {
        super();
        this.plan_id = "";
        this.plan_title = "";
        this.type_str = "";
        this.source = "";
        this.target = "";
        this.is_running = false;
        this.last_backup_time = "";
        this.last_backup_size = "";
        this.template_compiled = Handlebars.compile(templateContent);
    }

    setBackupPlan(plan_id: string, plan: BackupPlanInfo) {
        this.plan_id = plan_id
        this.plan_title = plan.title;
        this.type_str = plan.type_str;
        this.source = plan.source;
        this.target = plan.target;
        this.requestUpdate();
    }

    firstUpdated() {
        let backup_now_btn = this.shadowRoot?.querySelector("#backup-now-btn");
        if(backup_now_btn) {
            backup_now_btn.addEventListener("click", async () => {
                let new_task = await taskManager.createBackupTask(this.plan_id, null);
                taskManager.resumeBackupTask(new_task.taskid);
            });
        }
        
    }

    render() {
        let uidata = {
            plan_title: this.plan_title,
            type_str: this.type_str,
            source: this.source,
            target: this.target,
            is_running: this.is_running,
            last_backup_time: this.last_backup_time,
            last_backup_size: this.last_backup_size,
        };
        let real_content = this.template_compiled(uidata);
        //console.log(real_content);
        return html`${unsafeHTML(real_content)}`;
    }
  }
  
