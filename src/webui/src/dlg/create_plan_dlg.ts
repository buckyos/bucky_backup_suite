import templateContent from './create_plan_dlg.template?raw';
import {LitElement, html} from 'lit';
import {customElement, property} from 'lit/decorators.js';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import Handlebars from 'handlebars';
import { BuckyWizzardDlg } from '../components/wizzard_dlg';
import { SlInput } from '@shoelace-style/shoelace';
import { SelectTargetDlg } from './select_target_dlg';
import { taskManager } from '@/utils/task_mgr';

@customElement('create-plan-dlg')
class CreatePlanDlg extends LitElement {
    template_compiled: HandlebarsTemplateDelegate<any>;
    ownerWizzard: BuckyWizzardDlg;

    

    setOwnerWizzard(wizzard: BuckyWizzardDlg) {
        this.ownerWizzard = wizzard;
    }

    constructor() {
        super();
        //this.ownerWizzard = this.parentElement as BuckyWizzardDlg;
        this.template_compiled = Handlebars.compile(templateContent);
        this.ownerWizzard = this.parentElement as BuckyWizzardDlg;
        
    }

    async readDataFromUI() : Promise<boolean> {
        const description = this.shadowRoot?.querySelector("#description") as SlInput;
        if (description) {
            if (description.value.length < 5) {
                alert("Backup Description must be at least 5 characters long");
                return false;
            }
           this.ownerWizzard.wizzard_data.description = description.value;
        }
        const backup_source_path = this.shadowRoot?.querySelector("#backup-source-path") as SlInput;
        if (backup_source_path) {
            const path = backup_source_path.value.trim();
            if (!path) {
                alert("Please input the backup source path");
                return false;
            }

            
            try {
                // 发送请求到后端验证路径
                const path_exist = await taskManager.validatePath(path);
                
                if (!path_exist) {
                    alert("Invalid path, please confirm the path exists and has access permission");
                    return false;
                }
                
            } catch (error) {
                console.error('Path validation failed:', error);
                //alert("Path validation failed, please try again");
                //return false;
            }
            this.ownerWizzard.wizzard_data.backup_source_path = path;
        }
        return true;
    }

    firstUpdated() {
        const nextButton = this.shadowRoot?.querySelector("#next-button");
        if (nextButton) {
            nextButton.addEventListener("click", async () => {
                if (await this.readDataFromUI()) {
                    let select_target_dlg = document.createElement("select-target-dlg") as SelectTargetDlg;
                    if (this.ownerWizzard) {
                        select_target_dlg.setOwnerWizzard(this.ownerWizzard);
                        this.ownerWizzard.pushDlg(select_target_dlg,"Where to backup?");
                    }
                }
            });
        }
    }

    render() {
        let uidata = {

        };
        let real_content = this.template_compiled(uidata);
        return html`${unsafeHTML(real_content)}`;
    }
}