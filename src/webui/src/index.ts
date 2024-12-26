import { SlDialog , SlDropdown, SlAlert, SlMenu, SlButton} from "@shoelace-style/shoelace";

import "./components/bs_task_panel";
import "./components/bs_tasklist";
import "./components/panel_list";
import  "./components/bs_plan_panel";
import "./components/wizzard_dlg";
import "./dlg/create_plan_dlg";
import "./dlg/select_target_dlg";
import "./dlg/set_backup_timer_dlg";

import { PanelList } from "./components/panel_list";
import { BSTaskList, TaskFilter } from "./components/bs_tasklist";
import { BSPlanPanel } from "./components/bs_plan_panel";
import { taskManager, BackupPlanInfo,TaskInfo } from "./utils/task_mgr";
import { BSTaskPanel } from "./components/bs_task_panel";

enum PanelType {
    Home = "home",
    AllTask = "alltasks",
    Running = "running",
    Success = "success",
    Failed = "failed"
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
            panel.setBackupPlan(plan_id, plan);
            panel_list.add_panel(panel,plan_id);
        }
    } catch (error) {
        console.error("load plan list error:", error);
    }
}

//after dom loaded
window.onload = async () => {
    console.log("bucky backup suite webui loaded");
    let panel_type: PanelType | null = null;

    let switch_panel_list = async (to: PanelType): Promise<void> =>  {
        if (to == panel_type) {
            return;
        }

        panel_type = to;

        let tasklist = document.querySelector("#tasklist") as BSTaskList;
        let taskFilter: TaskFilter = "all";
        switch (panel_type) {
            case PanelType.Home:
                console.log("Switching to Home panel");
                taskFilter = "running";
                break;
            case PanelType.AllTask:
                console.log("Switching to All Tasks panel");
                taskFilter = "all";
                break;
            case PanelType.Running:
                console.log("Switching to Running Tasks panel");
                taskFilter = "running";
                break;
            case PanelType.Success:
                console.log("Switching to Success Tasks panel");
                taskFilter = "all";
                break;
            case PanelType.Failed:
                console.log("Switching to Failed Tasks panel");
                taskFilter = "paused";
                break;
        }
        tasklist.setTaskFilter(taskFilter);
    }
    

    taskManager.addTaskEventListener(async (event, data) => {
        console.log("get task event:", event, data);
        switch(event) {
        case "create_plan":
            let plan = data as BackupPlanInfo;
            let panel_list = document.querySelector("#panel-list") as PanelList;
            let panel = document.createElement('bs-plan-panel') as BSPlanPanel;
            panel.setBackupPlan(plan.plan_id,plan);
            panel_list.add_panel(panel, plan.plan_id);

            break;
        case "resume_task":
            let alert = document.createElement('sl-alert') as SlAlert;
            alert.innerHTML = "Backup task created and running...";
            alert.variant = "primary";
            alert.duration = 10000;
            alert.closable = true;
            document.body.appendChild(alert);
            break;
        }

        
    });

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

    let doModelAddPlan = async () => {
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
        const dlg = document.getElementById('create-backup-plan-dlg') as SlDialog;
        dlg.show();
    }

    if(panel_list) {
        panel_list.addEventListener("add-click", doModelAddPlan);
    }

    {
        let panel_type_dropdown = document.querySelector("#panel-type-dropdown") as SlDropdown;
        const menu = panel_type_dropdown.querySelector('sl-menu') as SlMenu;
        const button = panel_type_dropdown.querySelector('sl-button') as SlButton;
    
        menu.addEventListener('sl-select', (event) => {
            const selectedItem = event.detail.item;
            const value = selectedItem.value;
            button.textContent = selectedItem.textContent!.trim();

            switch_panel_list(value as PanelType);
        });
    }

    {
        let create_dropdown = document.querySelector("#create-dropdown") as SlDropdown;
        const menu = create_dropdown.querySelector('sl-menu') as SlMenu;
    
        menu.addEventListener('sl-select', (event) => {
            const selectedItem = event.detail.item;
            const value = selectedItem.value;
            switch (value) {
                case "newbackup":
                    doModelAddPlan();
                    break;
                case "restore":
                    break;
            }
        });
    }

    /**
     * 
     * 基本逻辑：
     * 加载任务列表
     * 加载Plan列表
     */
    await load_plan_list();
}
