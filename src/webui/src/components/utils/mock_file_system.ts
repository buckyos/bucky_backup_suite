export type MockFsSpec =
    | {
          kind: "dir";
          name: string;
          children: MockFsSpec[];
      }
    | {
          kind: "file";
          name: string;
          size: number;
          modifiedHoursAgo: number;
          chunkSizeRange?: [number, number];
      };

// 妯℃嫙鐨勭洰褰曟爲锛圝SON缁撴瀯锛岀洿瑙傛槗缂栬緫锛?
// 绾﹀畾锛氶敭涓烘枃浠跺す鍚嶏紝鍊间负鍏跺瓙鐩綍瀵硅薄锛涚┖瀵硅薄琛ㄧず璇ョ洰褰曠洰鍓嶆棤瀛愰」
export const MOCK_FILE_SYSTEM_SPEC: MockFsSpec = {
    kind: "dir",
    name: "/",
    children: [
        {
            kind: "dir",
            name: "C:",
            children: [
                {
                    kind: "dir",
                    name: "Users",
                    children: [
                        {
                            kind: "dir",
                            name: "Administrator",
                            children: [
                                {
                                    kind: "dir",
                                    name: "Desktop",
                                    children: [
                                        {
                                            kind: "file",
                                            name: "shortcut.lnk",
                                            size: 2048,
                                            modifiedHoursAgo: 6,
                                        },
                                        {
                                            kind: "file",
                                            name: "todo.txt",
                                            size: 1024,
                                            modifiedHoursAgo: 12,
                                        },
                                    ],
                                },
                                {
                                    kind: "dir",
                                    name: "Documents",
                                    children: [
                                        {
                                            kind: "dir",
                                            name: "Reports",
                                            children: [
                                                {
                                                    kind: "file",
                                                    name: "Q1_Report.docx",
                                                    size: 345600,
                                                    modifiedHoursAgo: 48,
                                                },
                                                {
                                                    kind: "file",
                                                    name: "Project_Status.pptx",
                                                    size: 1024000,
                                                    modifiedHoursAgo: 30,
                                                },
                                            ],
                                        },
                                        {
                                            kind: "file",
                                            name: "Budget.xlsx",
                                            size: 512000,
                                            modifiedHoursAgo: 72,
                                        },
                                        {
                                            kind: "file",
                                            name: "Notes.md",
                                            size: 18432,
                                            modifiedHoursAgo: 8,
                                        },
                                    ],
                                },
                                {
                                    kind: "dir",
                                    name: "Downloads",
                                    children: [
                                        {
                                            kind: "file",
                                            name: "installer.exe",
                                            size: 42000000,
                                            modifiedHoursAgo: 3,
                                        },
                                        {
                                            kind: "file",
                                            name: "manual.pdf",
                                            size: 4800000,
                                            modifiedHoursAgo: 10,
                                        },
                                        {
                                            kind: "file",
                                            name: "archive.zip",
                                            size: 120000000,
                                            modifiedHoursAgo: 50,
                                        },
                                    ],
                                },
                                {
                                    kind: "dir",
                                    name: "Pictures",
                                    children: [
                                        {
                                            kind: "dir",
                                            name: "Vacation",
                                            children: [
                                                {
                                                    kind: "file",
                                                    name: "IMG_0001.jpg",
                                                    size: 3200000,
                                                    modifiedHoursAgo: 200,
                                                },
                                                {
                                                    kind: "file",
                                                    name: "IMG_0002.jpg",
                                                    size: 3145728,
                                                    modifiedHoursAgo: 198,
                                                },
                                                {
                                                    kind: "file",
                                                    name: "IMG_0003.jpg",
                                                    size: 2980000,
                                                    modifiedHoursAgo: 197,
                                                },
                                            ],
                                        },
                                        {
                                            kind: "file",
                                            name: "avatar.png",
                                            size: 256000,
                                            modifiedHoursAgo: 24,
                                        },
                                    ],
                                },
                            ],
                        },
                        {
                            kind: "dir",
                            name: "Public",
                            children: [
                                {
                                    kind: "file",
                                    name: "readme.txt",
                                    size: 8192,
                                    modifiedHoursAgo: 480,
                                },
                            ],
                        },
                        {
                            kind: "dir",
                            name: "Default",
                            children: [
                                {
                                    kind: "file",
                                    name: "default.ini",
                                    size: 4096,
                                    modifiedHoursAgo: 960,
                                },
                            ],
                        },
                    ],
                },
                {
                    kind: "dir",
                    name: "Program Files",
                    children: [
                        {
                            kind: "dir",
                            name: "BuckySuite",
                            children: [
                                {
                                    kind: "file",
                                    name: "config.json",
                                    size: 9216,
                                    modifiedHoursAgo: 15,
                                },
                                {
                                    kind: "file",
                                    name: "bucky.exe",
                                    size: 23000000,
                                    modifiedHoursAgo: 400,
                                },
                            ],
                        },
                        {
                            kind: "dir",
                            name: "Common Files",
                            children: [
                                {
                                    kind: "file",
                                    name: "shared.dll",
                                    size: 8900000,
                                    modifiedHoursAgo: 520,
                                },
                            ],
                        },
                    ],
                },
                {
                    kind: "dir",
                    name: "Windows",
                    children: [
                        {
                            kind: "file",
                            name: "system32.dll",
                            size: 12000000,
                            modifiedHoursAgo: 720,
                        },
                    ],
                },
                {
                    kind: "dir",
                    name: "Temp",
                    children: [
                        {
                            kind: "file",
                            name: "tmp123.tmp",
                            size: 65536,
                            modifiedHoursAgo: 1,
                        },
                        {
                            kind: "file",
                            name: "cleanup.log",
                            size: 12288,
                            modifiedHoursAgo: 5,
                        },
                    ],
                },
            ],
        },
        {
            kind: "dir",
            name: "D:",
            children: [
                {
                    kind: "dir",
                    name: "Projects",
                    children: [
                        {
                            kind: "dir",
                            name: "Alpha",
                            children: [
                                {
                                    kind: "file",
                                    name: "README.md",
                                    size: 14336,
                                    modifiedHoursAgo: 90,
                                },
                                {
                                    kind: "file",
                                    name: "alpha.ts",
                                    size: 48576,
                                    modifiedHoursAgo: 70,
                                },
                                {
                                    kind: "file",
                                    name: "alpha.test.ts",
                                    size: 30720,
                                    modifiedHoursAgo: 65,
                                },
                            ],
                        },
                        {
                            kind: "dir",
                            name: "Beta",
                            children: [
                                {
                                    kind: "file",
                                    name: "design.pdf",
                                    size: 2400000,
                                    modifiedHoursAgo: 120,
                                },
                                {
                                    kind: "file",
                                    name: "notes.txt",
                                    size: 7168,
                                    modifiedHoursAgo: 72,
                                },
                            ],
                        },
                        {
                            kind: "dir",
                            name: "Gamma",
                            children: [
                                {
                                    kind: "dir",
                                    name: "src",
                                    children: [
                                        {
                                            kind: "file",
                                            name: "main.rs",
                                            size: 80000,
                                            modifiedHoursAgo: 40,
                                        },
                                        {
                                            kind: "file",
                                            name: "lib.rs",
                                            size: 52000,
                                            modifiedHoursAgo: 39,
                                        },
                                    ],
                                },
                                {
                                    kind: "dir",
                                    name: "docs",
                                    children: [
                                        {
                                            kind: "file",
                                            name: "overview.md",
                                            size: 24576,
                                            modifiedHoursAgo: 36,
                                        },
                                    ],
                                },
                            ],
                        },
                    ],
                },
                {
                    kind: "dir",
                    name: "Backups",
                    children: [
                        {
                            kind: "file",
                            name: "system_backup_2023_12_01.bak",
                            size: 350000000,
                            modifiedHoursAgo: 1200,
                        },
                        {
                            kind: "file",
                            name: "project_archive_2024_03_15.bak",
                            size: 210000000,
                            modifiedHoursAgo: 300,
                        },
                    ],
                },
                {
                    kind: "dir",
                    name: "Media",
                    children: [
                        {
                            kind: "dir",
                            name: "Music",
                            children: [
                                {
                                    kind: "file",
                                    name: "album.flac",
                                    size: 520000000,
                                    modifiedHoursAgo: 60,
                                },
                                {
                                    kind: "file",
                                    name: "single.mp3",
                                    size: 9800000,
                                    modifiedHoursAgo: 40,
                                },
                            ],
                        },
                        {
                            kind: "dir",
                            name: "Videos",
                            children: [
                                {
                                    kind: "file",
                                    name: "presentation.mp4",
                                    size: 1500000000,
                                    modifiedHoursAgo: 24,
                                },
                                {
                                    kind: "file",
                                    name: "demo.mov",
                                    size: 680000000,
                                    modifiedHoursAgo: 18,
                                },
                            ],
                        },
                        {
                            kind: "dir",
                            name: "Photos",
                            children: [
                                {
                                    kind: "file",
                                    name: "wedding.jpg",
                                    size: 4200000,
                                    modifiedHoursAgo: 300,
                                },
                                {
                                    kind: "file",
                                    name: "family.png",
                                    size: 3500000,
                                    modifiedHoursAgo: 260,
                                },
                            ],
                        },
                    ],
                },
                {
                    kind: "dir",
                    name: "Data",
                    children: [
                        {
                            kind: "file",
                            name: "analytics.db",
                            size: 820000000,
                            modifiedHoursAgo: 12,
                        },
                        {
                            kind: "file",
                            name: "metrics.csv",
                            size: 5500000,
                            modifiedHoursAgo: 6,
                        },
                    ],
                },
            ],
        },
        {
            kind: "dir",
            name: "E:",
            children: [
                {
                    kind: "dir",
                    name: "Exchange",
                    children: [
                        {
                            kind: "file",
                            name: "drop_here.txt",
                            size: 1024,
                            modifiedHoursAgo: 2,
                        },
                    ],
                },
            ],
        },
    ],
};
