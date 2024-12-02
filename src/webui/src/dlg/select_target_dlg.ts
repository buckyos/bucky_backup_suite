import templateContent from './select_target_dlg.template?raw';
import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import { BuckyWizzardDlg } from '../components/wizzard_dlg';
import { taskManager } from '../utils/task_mgr';
import { SlInput } from '@shoelace-style/shoelace';
import { SetBackupTimerDlg } from './set_backup_timer_dlg';

@customElement('select-target-dlg')
class SelectTargetDlg extends LitElement {
    template_compiled: HandlebarsTemplateDelegate<any>;
    ownerWizzard: BuckyWizzardDlg;

    setOwnerWizzard(wizzard: BuckyWizzardDlg) {
        this.ownerWizzard = wizzard;
    }

    constructor() {
        super();
        this.ownerWizzard =  null;
        this.template_compiled = Handlebars.compile(templateContent);
    }

    async readDataFromUI() {
        let backup_target_path = this.shadowRoot?.querySelector("#backup-target-path") as SlInput;

        if (backup_target_path) {
            if (backup_target_path.value.length < 5) {
                alert("Backup target path must be at least 5 characters long");
                return false;
            }
            try {
                const path_exist = await taskManager.validatePath(backup_target_path.value);
                if (!path_exist) {
                alert("Invalid path, please confirm the path exists and has access permission");
                    return false;
                }
            } catch (error) {
                console.error('Path validation failed:', error);
                //alert("Path validation failed, please try again");
                //return false;
            }
            this.ownerWizzard.wizzard_data.backup_target_path = backup_target_path.value;
        }
        
        return true;
    }

    firstUpdated() {
        const nextButton = this.shadowRoot?.querySelector("#next-button");
        if (nextButton) {
            nextButton.addEventListener("click", async () => {
                if (await this.readDataFromUI()) {      
                    let set_backup_timer_dlg = document.createElement("set-backup-timer-dlg") as SetBackupTimerDlg;
                    set_backup_timer_dlg.setOwnerWizzard(this.ownerWizzard);
                    this.ownerWizzard.pushDlg(set_backup_timer_dlg,"When to run backups?");
                }
            });
        }
    }

    render() {
        let uidata = {

        };
        let real_content = this.template_compiled(uidata);
        //console.log(real_content);
        return html`${unsafeHTML(real_content)}`;
    }
}