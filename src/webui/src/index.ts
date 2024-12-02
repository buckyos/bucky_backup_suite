import { SlDialog , SlDropdown} from "@shoelace-style/shoelace";
import "./components/bs_task_panel";
import "./components/bs_tasklist";
import "./components/panel_list";
import "./components/wizzard_dlg";

import "./dlg/create_plan_dlg";
import "./dlg/select_target_dlg";
import "./dlg/set_backup_timer_dlg";

//after dom loaded
window.onload = async () => {
    console.log("bucky backup suite webui loaded");
 
    let tasklist = document.querySelector("#tasklist");
    let panel_list = document.querySelector("#panel-list");

    if(panel_list) {
        panel_list.addEventListener("add-click", async () => {
            const dialog = document.createElement('sl-dialog') as SlDialog;
            dialog.id = 'create-backup-plan-dlg';
            dialog.setAttribute('no-header', '');
            dialog.setAttribute('overlay-dismiss', 'false');
            dialog.id = 'create-backup-plan-dlg';
            dialog.innerHTML = `
                <bucky-wizzard-dlg id="create-wizzard" title="Create Backup Plan">
                    <create-plan-dlg></create-plan-dlg>
                </bucky-wizzard-dlg>
            `;
        
            dialog.addEventListener('sl-request-close', event => {
                //console.log("sl-request-close");
                //if (event.detail.source === 'overlay') {
                  event.preventDefault();
                //}
            });
            //console.log(dialog);
            //await dialog.show();
            dialog.addEventListener('sl-after-hide', () => {
                dialog.remove();
            });

            document.body.appendChild(dialog);
            //sleep 10ms
            setTimeout(() => {
                const dlg = document.getElementById('create-backup-plan-dlg') as SlDialog;
                dlg.show();
            }, 15);
        });
    }
    /**
     * 
     * 基本逻辑：
     * 加载任务列表
     * 加载Plan列表
     */
}