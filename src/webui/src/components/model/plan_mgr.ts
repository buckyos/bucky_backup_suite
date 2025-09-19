export const test_data_plans = [
    {
        id: 1,
        name: "系统文件备份",
        description: "每日自动备份系统关键文件",
        enabled: true,
        source: "C:\\Windows\\System32",
        destination: "本地D盘",
        nextRun: "今天 23:00",
        schedule: "每天 23:00",
        lastRun: "昨天 23:00",
        status: "healthy",
    },
    {
        id: 2,
        name: "项目文件备份",
        description: "工作项目的增量备份",
        enabled: true,
        source: "D:\\Projects",
        destination: "NDN网络",
        nextRun: "明天 02:00",
        schedule: "每周一、三、五 02:00",
        lastRun: "2天前",
        status: "healthy",
    },
    {
        id: 3,
        name: "文档备份",
        description: "个人文档和配置文件",
        enabled: false,
        source: "C:\\Users\\Documents",
        destination: "本地D盘",
        nextRun: "已禁用",
        schedule: "每天 01:00",
        lastRun: "1周前",
        status: "disabled",
    },
    {
        id: 4,
        name: "媒体文件备份",
        description: "照片和视频文件备份",
        enabled: true,
        source: "D:\\Media",
        destination: "NDN网络",
        nextRun: "今天 20:00",
        schedule: "每天 20:00",
        lastRun: "昨天 20:00",
        status: "warning",
    },
];

export class PlanManager {
    // Plan management logic here
}
