import { SlDialog , SlDropdown, SlAlert, SlMenu, SlButton} from "@shoelace-style/shoelace";

import "./components/bs_task_panel";
import "./components/bs_tasklist";
import "./components/panel_list";
import  "./components/bs_plan_panel";
import "./components/wizzard_dlg";
import "./dlg/create_plan_dlg";
import "./dlg/select_target_dlg";
import "./dlg/set_backup_timer_dlg";
import "./dlg/restore_checkpoint_dlg";
import "./dlg/restore_select_target_dlg";
import './components/bs_s3_config';

import { PanelList } from "./components/panel_list";
import { BSTaskList } from "./components/bs_tasklist";
import { BSPlanPanel } from "./components/bs_plan_panel";
import { taskManager, BackupPlanInfo,TaskInfo, TaskFilter } from "./utils/task_mgr";
import { BSTaskPanel } from "./components/bs_task_panel";

enum TaskPanelType {
    Home = "home",
    AllTask = "alltasks",
    Running = "running",
    Success = "success",
    Paused = "paused",
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
    let task_panel_type: TaskPanelType | null = null;

    let switch_task_list = async (to: TaskPanelType): Promise<void> =>  {
        if (to == task_panel_type) {
            return;
        }

        task_panel_type = to;

        let tasklist = document.querySelector("#tasklist") as BSTaskList;
        let panel_type_dropdown = document.querySelector("#panel-type-dropdown") as SlDropdown;
        const button = panel_type_dropdown.querySelector('sl-button') as SlButton;
        let taskFilter: TaskFilter = "all";
        switch (task_panel_type) {
            case TaskPanelType.Home:
                console.log("Switching to Home panel");
                taskFilter = "running";
                button.textContent = "Home";
                break;
            case TaskPanelType.AllTask:
                console.log("Switching to All Tasks panel");
                taskFilter = "all";
                button.textContent = "All Tasks";
                break;
            case TaskPanelType.Running:
                console.log("Switching to Running Tasks panel");
                taskFilter = "running";
                button.textContent = "Running Tasks";
                break;
            case TaskPanelType.Success:
                console.log("Switching to Success Tasks panel");
                taskFilter = "done";
                button.textContent = "Success Tasks";
                break;
            case TaskPanelType.Paused:
                console.log("Switching to Paused Tasks panel");
                taskFilter = "paused";
                button.textContent = "Paused Tasks";
                break;
            case TaskPanelType.Failed:
                console.log("Switching to Failed Tasks panel");
                taskFilter = "failed";
                button.textContent = "Failed Tasks";
                break;
        }
        console.log("switch_task_list:", taskFilter, button.textContent);
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
        dialog.addEventListener('sl-after-hide', (event) => {
            if (event.target === dialog) {
                dialog.remove();
            }
        });

        document.body.appendChild(dialog);
        const dlg = document.getElementById('create-backup-plan-dlg') as SlDialog;
        dlg.show();
    }

    let doModelRestore = async () => {
        console.log("doModelRestore");
        const dialog = document.createElement('sl-dialog') as SlDialog;
        dialog.id = 'create-restore-task-dlg';
        dialog.setAttribute('no-header', '');
        dialog.setAttribute('overlay-dismiss', 'false');
        dialog.innerHTML = `
            <bucky-wizzard-dlg id="create-restore-task-dlg" title="Create Restore Task">
                <restore-checkpoint-dlg></restore-checkpoint-dlg>
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
        dialog.addEventListener('sl-after-hide', (event) => {
            // console.log("event.composedPath():", event.composedPath());
            if (event.target === dialog) {
                dialog.remove();
            }
        });

        document.body.appendChild(dialog);
        const dlg = document.getElementById('create-restore-task-dlg') as SlDialog;
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

            switch_task_list(value as TaskPanelType);
        });
    }

    {
        let create_dropdown = document.querySelector("#create-dropdown") as SlDropdown;
        const menu = create_dropdown.querySelector('sl-menu') as SlMenu;
    
        const create_backup_menu = document.querySelector("#create-backup") as SlMenu;

        menu.addEventListener('sl-select', async (event) => {
            const selectedItem = event.detail.item;
            const value = selectedItem.value;
            switch (value) {
                case "newplan":
                    doModelAddPlan();
                    break;
                case "restore":
                    doModelRestore();
                    break;
            }
        });

        let new_backup_menuitem = document.getElementById('newbackup-menuitem')!;
        let new_plan_menuitem = document.getElementById('newplan-menuitem')!;
        let new_plan_restore = document.getElementById('restore-menuitem')!;
        let plan_list_dropdown = document.querySelector('#plan-list-dropdown') as SlDropdown;
        new_backup_menuitem.addEventListener('mouseenter', async () => {
            create_backup_menu.innerHTML = '';
            let planid_list = await taskManager.listBackupPlans() as string[];
            let plan_list = await Promise.all(
                planid_list.map((plan_id) => 
                    taskManager.getBackupPlan(plan_id)
                )
            )
            for(let plan of plan_list) {
                console.log("plan_id:", plan.plan_id);
                const newItem = document.createElement('sl-menu-item');
                newItem.textContent = plan.title;
                newItem.value = plan.plan_id;
                create_backup_menu.appendChild(newItem);
            }
            plan_list_dropdown.open = true;
            plan_list_dropdown.style.left = `${new_backup_menuitem.offsetWidth}px`;
        });

        let close_plan_list_dropdown_items = [new_plan_menuitem, new_plan_restore];
        close_plan_list_dropdown_items.forEach((item) => {
            item.addEventListener('mouseenter', function() {
                plan_list_dropdown.open = false;
            });
        })
    
        create_backup_menu.addEventListener('sl-select', async (event) => {
            const selectedItem = event.detail.item;
            const plan_id = selectedItem.value;
            let new_task = await taskManager.createBackupTask(plan_id, null);
            taskManager.resumeBackupTask(new_task.taskid);
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
