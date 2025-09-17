import templateContent from "./restore_checkpoint_dlg.template?raw";
import { LitElement, html } from "lit";
import { customElement, property } from "lit/decorators.js";
import { unsafeHTML } from "lit/directives/unsafe-html.js";
import Handlebars from "handlebars";
import { BuckyWizzardDlg } from "../components/wizzard_dlg";
import { SlInput, SlSelect } from "@shoelace-style/shoelace";
import { taskManager } from "@/utils/task_mgr";
import { RestoreSelectTargetDlg } from "./restore_select_target_dlg";

@customElement("restore-checkpoint-dlg")
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

    async readDataFromUI(): Promise<boolean> {
        const plan_combo = this.shadowRoot?.querySelector(
            "#backup-plan-combo"
        ) as SlSelect;
        if (plan_combo) {
            if (plan_combo.selectedOptions.length < 1) {
                alert("You should select a backup plan");
                return false;
            }
            this.ownerWizzard.wizzard_data.plan_id =
                plan_combo.selectedOptions[0].value;
        }
        const checkpoint_combo = this.shadowRoot?.querySelector(
            "#checkpoint-combo"
        ) as SlSelect;
        if (checkpoint_combo) {
            if (checkpoint_combo.selectedOptions.length < 1) {
                alert("You should select a checkpoint");
                return false;
            }
            this.ownerWizzard.wizzard_data.checkpoint_id =
                checkpoint_combo.selectedOptions[0].value;
        }
        return true;
    }

    firstUpdated() {
        const backupPlanCombo = this.shadowRoot?.querySelector(
            "#backup-plan-combo"
        ) as SlSelect;
        const checkpointContainer = this.shadowRoot?.querySelector(
            "#checkpoint-container"
        ) as HTMLElement;
        const checkpointCombo = this.shadowRoot?.querySelector(
            "#checkpoint-combo"
        ) as SlSelect;

        let select_plan_id: null | string = null;

        backupPlanCombo.addEventListener("sl-change", (event) => {
            let eventTarget = event.target as HTMLSelectElement;
            if (event.target) {
                if (backupPlanCombo.value) {
                    checkpointContainer.style.display = "block";
                }

                if (eventTarget.value != select_plan_id) {
                    select_plan_id = eventTarget.value;
                    load_checkpoint_list(eventTarget.value);
                }
            }
        });

        const nextButton = this.shadowRoot?.querySelector("#next-button");
        if (nextButton) {
            nextButton.addEventListener("click", async () => {
                if (await this.readDataFromUI()) {
                    let select_target_dlg = document.createElement(
                        "restore-select-target-dlg"
                    ) as RestoreSelectTargetDlg;
                    if (this.ownerWizzard) {
                        select_target_dlg.setOwnerWizzard(this.ownerWizzard);
                        this.ownerWizzard.pushDlg(
                            select_target_dlg,
                            "Where to restore?"
                        );
                    }
                }
            });
        }

        let load_plan_list = async () => {
            try {
                let plan_ids =
                    (await taskManager.listBackupPlans()) as string[];
                let plans = await Promise.all(
                    plan_ids.map((plan_id) =>
                        taskManager.getBackupPlan(plan_id)
                    )
                );
                plans.forEach((plan) => {
                    console.log("plan_id:", plan.plan_id, "plan:", plan);
                    const option = document.createElement("sl-option");
                    option.value = plan.plan_id;
                    option.textContent = plan.title;
                    backupPlanCombo.appendChild(option);
                });
            } catch (error) {
                console.error("load plan list error:", error);
            }
        };

        let load_checkpoint_list = async (plan_id: string) => {
            try {
                checkpointCombo.innerHTML = "";
                let task_ids = (await taskManager.listBackupTasks(
                    "done"
                )) as string[];
                let tasks = await Promise.all(
                    task_ids.map((task_id) => taskManager.getTaskInfo(task_id))
                );
                console.log("load checkpoint:", tasks);
                tasks
                    .filter(
                        (task) =>
                            task.state == "DONE" &&
                            task.owner_plan_id == plan_id &&
                            task.task_type != "RESTORE"
                    )
                    .forEach((task) => {
                        console.log(
                            "checkpoint_id:",
                            task.checkpoint_id,
                            "task:",
                            task
                        );
                        const option = document.createElement("sl-option");
                        option.value = task.checkpoint_id;
                        option.textContent = new Date(
                            task.update_time
                        ).toLocaleString();
                        checkpointCombo.appendChild(option);
                    });
            } catch (error) {
                console.error("load plan list error:", error);
            }
        };

        load_plan_list();
    }

    render() {
        let uidata = {};
        let real_content = this.template_compiled(uidata);
        return html`${unsafeHTML(real_content)}`;
    }
}
