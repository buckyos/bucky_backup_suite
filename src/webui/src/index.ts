import { SlDialog , SlDropdown, SlAlert} from "@shoelace-style/shoelace";

import "./components/bs_task_panel";
import "./components/bs_tasklist";
import "./components/panel_list";
import  "./components/bs_plan_panel";
import "./components/wizzard_dlg";
import "./dlg/create_plan_dlg";
import "./dlg/select_target_dlg";
import "./dlg/set_backup_timer_dlg";

import { PanelList } from "./components/panel_list";
import { BSTaskList } from "./components/bs_tasklist";
import { BSPlanPanel } from "./components/bs_plan_panel";
import { taskManager } from "./utils/task_mgr";

async function load_task_list(filter:string = "all") {
    let tasklist = await taskManager.listBackupTasks();
    console.log("tasklist:", tasklist);

}

async function load_plan_list() {
    try {
        let panel_list = document.querySelector("#panel-list") as PanelList;
        panel_list.clear_panels();
        let plans = await taskManager.listBackupPlans();
        for(let plan_id of plans) {
            console.log("plan_id:", plan_id);
            let plan = await taskManager.getBackupPlan(plan_id);
            let panel = document.createElement('bs-plan-panel') as BSPlanPanel;
            panel.plan_id = plan_id;
            panel.plan_title = plan.title;
            panel.type_str = plan.type_str;
            panel.source = plan.source;
            panel.target = plan.target;
            panel.is_running = plan.is_running;
            panel_list.add_panel(panel,plan_id);
        }
    } catch (error) {
        console.error("load plan list error:", error);
    }
}


//after dom loaded
window.onload = async () => {
    console.log("bucky backup suite webui loaded");
    
    taskManager.addTaskEventListener((event, data) => {
        console.log("task event:", event, data);
        if(event == "resume_task") {
            let alert = document.createElement('sl-alert') as SlAlert;
            alert.innerHTML = "Backup task created and running...";
            alert.variant = "primary";
            alert.duration = 10000;
            alert.closable = true;
            document.body.appendChild(alert);
        }
    });

    let tasklist = document.querySelector("#tasklist") as BSTaskList;
    let panel_list = document.querySelector("#panel-list") as PanelList;
    const resumeButton = document.getElementById('resume-button');
    resumeButton?.addEventListener('click', async () => {
        console.log("Resuming last working task...");
        await taskManager.resume_last_working_task();
    });

    const pauseAllButton = document.getElementById('pause-all-button');
    pauseAllButton?.addEventListener('click', async () => {
        console.log("Pausing all tasks...");
        await taskManager.pause_all_tasks();
    });

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
    await load_plan_list();
    await load_task_list();
}
