import templateContent from './select_target_dlg.template?raw';
import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import { BuckyWizzardDlg } from '../components/wizzard_dlg';
import { taskManager } from '../utils/task_mgr';
import { SlInput, SlSelect } from '@shoelace-style/shoelace';
import { SetBackupTimerDlg } from './set_backup_timer_dlg';
import { BSS3Config } from '../components/bs_s3_config';

enum TargetType {
    Local = "local",
    S3 = "s3"
}

@customElement('select-target-dlg')
export class SelectTargetDlg extends LitElement {
    template_compiled: HandlebarsTemplateDelegate<any>;
    ownerWizzard: BuckyWizzardDlg | null;
    currentTargetType: TargetType = TargetType.Local;

    setOwnerWizzard(wizzard: BuckyWizzardDlg) {
        this.ownerWizzard = wizzard;
    }

    constructor() {
        super();
        this.ownerWizzard = null;
        this.template_compiled = Handlebars.compile(templateContent);
    }

    async readDataFromUI() {
        if (this.currentTargetType === TargetType.Local) {
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
                }
                if (this.ownerWizzard) {
                    this.ownerWizzard.wizzard_data.backup_target_type = "local_chunk";
                    this.ownerWizzard.wizzard_data.backup_target_url = `file:///${backup_target_path.value}`;
                }
            }
        } else if (this.currentTargetType === TargetType.S3) {
            const s3Config = this.shadowRoot?.querySelector("#s3-target-config") as BSS3Config;
            if (s3Config) {
                if (this.ownerWizzard) {
                    this.ownerWizzard.wizzard_data.backup_target_type = "s3_chunk";
                    this.ownerWizzard.wizzard_data.backup_target_url = s3Config.getUrl();
                }
            }
        }
        return true;
    }

    firstUpdated() {
        const targetTypeSelect = this.shadowRoot?.querySelector("#target-type") as SlSelect;
        if (targetTypeSelect) {
            targetTypeSelect.addEventListener("sl-change", (e: any) => {
                this.currentTargetType = e.target.value as TargetType;
                this.updateTargetConfigVisibility();
            });
        }

        const nextButton = this.shadowRoot?.querySelector("#next-button");
        if (nextButton) {
            nextButton.addEventListener("click", async () => {
                if (await this.readDataFromUI()) {      
                    let set_backup_timer_dlg = document.createElement("set-backup-timer-dlg") as SetBackupTimerDlg;
                    if (this.ownerWizzard) {
                        set_backup_timer_dlg.setOwnerWizzard(this.ownerWizzard);
                        this.ownerWizzard.pushDlg(set_backup_timer_dlg,"When to run backups?");
                    }
                }
            });
        }
    }

    private updateTargetConfigVisibility() {
        const localConfig = this.shadowRoot?.querySelector("#local-config");
        const s3Config = this.shadowRoot?.querySelector("#s3-config");
        
        if (localConfig && s3Config) {
            if (this.currentTargetType === TargetType.Local) {
                localConfig.classList.remove("hidden");
                s3Config.classList.add("hidden");
            } else {
                localConfig.classList.add("hidden");
                s3Config.classList.remove("hidden");
            }
        }
    }

    render() {
        let uidata = {};
        let real_content = this.template_compiled(uidata);
        return html`${unsafeHTML(real_content)}`;
    }
}