export const test_data_targets = [
    {
        id: 1,
        name: "本地备份盘",
        type: "local",
        path: "D:\\Backups",
        status: "healthy",
        used: "450 GB",
        total: "2 TB",
        compression: true,
        encryption: true,
    },
    {
        id: 2,
        name: "NDN网络节点1",
        type: "ndn",
        endpoint: "ndn://backup.example.com",
        status: "healthy",
        used: "1.2 TB",
        total: "无限制",
        compression: true,
        encryption: true,
    },
    {
        id: 3,
        name: "外部硬盘",
        type: "local",
        path: "E:\\Backups",
        status: "warning",
        used: "1.8 TB",
        total: "2 TB",
        compression: false,
        encryption: true,
    },
    {
        id: 4,
        name: "NDN网络节点2",
        type: "ndn",
        endpoint: "ndn://backup2.example.com",
        status: "offline",
        used: "0 GB",
        total: "无限制",
        compression: true,
        encryption: false,
    },
];

export class TargetMgr {
    // Target management logic here
}
