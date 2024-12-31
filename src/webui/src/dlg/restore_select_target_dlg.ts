import templateContent from './restore_select_target_dlg.template?raw';
import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import { BuckyWizzardDlg } from '../components/wizzard_dlg';
import { taskManager } from '../utils/task_mgr';
import { SlCheckbox, SlInput, SlSelect } from '@shoelace-style/shoelace';

@customElement('restore-select-target-dlg')
export class RestoreSelectTargetDlg extends LitElement {
    template_compiled: HandlebarsTemplateDelegate<any>;
    ownerWizzard: BuckyWizzardDlg | null;

    setOwnerWizzard(wizzard: BuckyWizzardDlg) {
        this.ownerWizzard = wizzard;
    }

    constructor() {
        super();
        this.ownerWizzard = null;
        this.template_compiled = Handlebars.compile(templateContent);
    }

    async readDataFromUI() {
        let restore_target_path = this.shadowRoot?.querySelector("#restore-target-path") as SlInput;
        if (restore_target_path.value.length < 5) {
            alert("Restore target path must be at least 5 characters long");
            return false;
        }
        try {
            const path_exist = await taskManager.validatePath(restore_target_path.value);
            if (!path_exist) {
                alert("Invalid path, please confirm the path exists and has access permission");
                return false;
            }
        } catch (error) {
            console.error('Path validation failed:', error);
        }
        if (this.ownerWizzard) {
            this.ownerWizzard.wizzard_data.restore_target_url = `file:///${restore_target_path.value}`;
        }

        let is_clean_folder = this.shadowRoot?.querySelector("#is-clean-folder") as SlCheckbox;
        if (is_clean_folder) {
            this.ownerWizzard!.wizzard_data.is_clean_folder = is_clean_folder.checked;
        }

        return true;
    }

    firstUpdated() {
        this.shadowRoot?.querySelector("#restore-btn")?.addEventListener("click", async () => {
            console.log("restore clicked");
            await this.readDataFromUI();
            console.log(this.ownerWizzard!.wizzard_data);
            // let restoreConfig = {
            //     type_str: "c2c",
            //     source_type: "local_chunk",
            //     source: "file:///" + this.ownerWizzard.wizzard_data.backup_source_path,
            //     target_type: this.ownerWizzard.wizzard_data.backup_target_type,
            //     target: this.ownerWizzard.wizzard_data.backup_target_url,
            //     title: this.ownerWizzard.wizzard_data.description,
            //     description: this.ownerWizzard.wizzard_data.description,
            // }
            
            let restore_task = await taskManager.createRestoreTask(
                this.ownerWizzard!.wizzard_data.plan_id,
                this.ownerWizzard!.wizzard_data.checkpoint_id,
                this.ownerWizzard!.wizzard_data.restore_target_url,
                this.ownerWizzard!!.wizzard_data.is_clean_folder
            );
            console.log("restore_task: ", restore_task);
            taskManager.resumeBackupTask(restore_task.taskid);
            this.ownerWizzard!.closeDlg();
        });
    }

    render() {
        let uidata = {};
        let real_content = this.template_compiled(uidata);
        return html`${unsafeHTML(real_content)}`;
    }
}