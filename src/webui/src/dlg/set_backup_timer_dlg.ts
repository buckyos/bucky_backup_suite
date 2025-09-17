import templateContent from './set_backup_timer_dlg.template?raw';
import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import { SlCheckbox } from '@shoelace-style/shoelace';
import { taskManager } from '../utils/task_mgr';
import { BuckyWizzardDlg } from '@/components/wizzard_dlg';

@customElement('set-backup-timer-dlg')
export class SetBackupTimerDlg extends LitElement {
    template_compiled: HandlebarsTemplateDelegate<any>;
    ownerWizzard: BuckyWizzardDlg;
    constructor() {
        super();
        this.ownerWizzard = null;
        this.template_compiled = Handlebars.compile(templateContent);
    }

    setOwnerWizzard(wizzard: BuckyWizzardDlg) {
        this.ownerWizzard = wizzard;
    }

    firstUpdated() {
        this.shadowRoot?.querySelector("#create-btn")?.addEventListener("click", async () => {
            let is_run_now = this.shadowRoot?.querySelector("#is-run-now") as SlCheckbox;
            if (is_run_now) {
                this.ownerWizzard.wizzard_data.is_run_now = is_run_now.checked;
            }
            console.log(this.ownerWizzard.wizzard_data);
            let planConfig = {
                type_str: "c2c",
                source_type: "local_chunk",
                source: "file:///" + this.ownerWizzard.wizzard_data.backup_source_path,
                target_type: this.ownerWizzard.wizzard_data.backup_target_type,
                target: this.ownerWizzard.wizzard_data.backup_target_url,
                title: this.ownerWizzard.wizzard_data.description,
                description: this.ownerWizzard.wizzard_data.description,
            }
            
            let plan_id = await taskManager.createBackupPlan(planConfig);
            console.log("plan_id: ", plan_id);
            this.ownerWizzard.closeDlg();
        });
    }

    render() {
        let uidata = {

        };
        let real_content = this.template_compiled(uidata);
        //console.log(real_content);
        return html`${unsafeHTML(real_content)}`;
    }
}