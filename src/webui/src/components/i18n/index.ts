export interface Translations {
  common: {
    save: string;
    cancel: string;
    delete: string;
    edit: string;
    add: string;
    search: string;
    filter: string;
    all: string;
    status: string;
    type: string;
    name: string;
    actions: string;
    details: string;
    back: string;
    next: string;
    previous: string;
    finish: string;
    loading: string;
    error: string;
    success: string;
    warning: string;
    info: string;
    create: string;
    run: string;
    logs: string;
    restore: string;
    connection: string;
    test: string;
    enabled: string;
    disabled: string;
    required: string;
    optional: string;
    today: string;
    tomorrow: string;
    minutes: string;
    hours: string;
    days: string;
    weeks: string;
    page: string;
    of: string;
    showing: string;
    results: string;
  };
  nav: {
    dashboard: string;
    plans: string;
    services: string;
    tasks: string;
    settings: string;
  };
  dashboard: {
    title: string;
    subtitle: string;
    activeTasks: string;
    totalBackupSize: string;
    backupPlans: string;
    successRate: string;
    currentTasks: string;
    recentActivities: string;
    backupServices: string;
    createNewPlan: string;
    backupNow: string;
    addService: string;
    viewAll: string;
  };
  plans: {
    title: string;
    subtitle: string;
    createNew: string;
    enabled: string;
    disabled: string;
    source: string;
    destination: string;
    schedule: string;
    nextRun: string;
    lastRun: string;
    runNow: string;
    viewLogs: string;
    healthy: string;
    warning: string;
    triggerType: string;
    scheduled: string;
    eventTriggered: string;
    manualOnly: string;
    schedulePeriod: string;
    daily: string;
    weekly: string;
    monthly: string;
    eventDelay: string;
    planName: string;
    planDescription: string;
    selectDirectories: string;
    selectService: string;
    backupType: string;
    fullBackup: string;
    incrementalBackup: string;
    versionsToKeep: string;
    priority: string;
    high: string;
    medium: string;
    low: string;
    basicInfo: string;
    directories: string;
    service: string;
    trigger: string;
    advanced: string;
    review: string;
    overview: string;
  };
  services: {
    title: string;
    subtitle: string;
    addService: string;
    local: string;
    ndn: string;
    online: string;
    offline: string;
    testConnection: string;
    storageUsed: string;
    compression: string;
    encryption: string;
    enabled: string;
    disabled: string;
    serviceName: string;
    serviceType: string;
    storageConfig: string;
    serverConfig: string;
    storagePath: string;
    serverUrl: string;
    username: string;
    password: string;
    accessKey: string;
    secretKey: string;
    bucket: string;
    region: string;
    endpoint: string;
  };
  tasks: {
    title: string;
    subtitle: string;
    running: string;
    completed: string;
    paused: string;
    failed: string;
    queued: string;
    backup: string;
    restore: string;
    pause: string;
    resume: string;
    stop: string;
    progress: string;
    speed: string;
    remaining: string;
    allTasks: string;
    runningTasks: string;
    filterTasks: string;
    searchPlaceholder: string;
    planName: string;
    taskType: string;
    startTime: string;
    endTime: string;
    duration: string;
    dataSize: string;
    createRestore: string;
    restoreTitle: string;
    restoreSubtitle: string;
    selectBackup: string;
    restoreTarget: string;
    selectTarget: string;
    originalLocation: string;
    customLocation: string;
    overwriteFiles: string;
    skipExisting: string;
  };
  settings: {
    title: string;
    subtitle: string;
    general: string;
    notifications: string;
    security: string;
    performance: string;
    advanced: string;
    language: string;
    timezone: string;
    autoStart: string;
    minimizeToTray: string;
    autoUpdate: string;
    lightMode: string;
    darkMode: string;
  };
}

const zhTranslations: Translations = {
  common: {
    save: '保存',
    cancel: '取消',
    delete: '删除',
    edit: '编辑',
    add: '添加',
    search: '搜索',
    filter: '筛选',
    all: '全部',
    status: '状态',
    type: '类型',
    name: '名称',
    actions: '操作',
    details: '详情',
    back: '返回',
    next: '下一步',
    previous: '上一步',
    finish: '完成',
    loading: '加载中',
    error: '错误',
    success: '成功',
    warning: '警告',
    info: '信息',
    create: '创建',
    run: '执行',
    logs: '日志',
    restore: '恢复',
    connection: '连接',
    test: '测试',
    enabled: '已启用',
    disabled: '已禁用',
    required: '必填',
    optional: '可选',
    today: '今天',
    tomorrow: '明天',
    minutes: '分钟',
    hours: '小时',
    days: '天',
    weeks: '周',
    page: '页',
    of: '共',
    showing: '显示',
    results: '条结果',
  },
  nav: {
    dashboard: '首页',
    plans: '备份计划',
    services: '服务管理',
    tasks: '任务列表',
    settings: '设置',
  },
  dashboard: {
    title: '仪表盘',
    subtitle: '备份系统概览和快速操作',
    activeTasks: '活跃任务',
    totalBackupSize: '总备份大小',
    backupPlans: '备份计划',
    successRate: '成功率',
    currentTasks: '当前任务',
    recentActivities: '近期活动',
    backupServices: '备份服务',
    createNewPlan: '创建新计划',
    backupNow: '立即备份',
    addService: '添加服务',
    viewAll: '查看全部',
  },
  plans: {
    title: '备份计划',
    subtitle: '管理和配置自动备份计划',
    createNew: '创建新计划',
    enabled: '已启用',
    disabled: '已禁用',
    source: '备份源',
    destination: '目标位置',
    schedule: '计划时间',
    nextRun: '下次执行',
    lastRun: '上次执行',
    runNow: '立即执行',
    viewLogs: '查看日志',
    healthy: '正常',
    warning: '警告',
    triggerType: '触发方式',
    scheduled: '定时执行',
    eventTriggered: '事件触发',
    manualOnly: '仅手动',
    schedulePeriod: '调度周期',
    daily: '每天',
    weekly: '每周',
    monthly: '每月',
    eventDelay: '触发延迟',
    planName: '计划名称',
    planDescription: '计划描述',
    selectDirectories: '选择目录',
    selectService: '选择服务',
    backupType: '备份类型',
    fullBackup: '完全备份',
    incrementalBackup: '增量备份',
    versionsToKeep: '版本保留',
    priority: '优先级',
    high: '高',
    medium: '中',
    low: '低',
    basicInfo: '基本信息',
    directories: '目录',
    service: '服务',
    trigger: '触发',
    advanced: '高级',
    review: '确认',
    overview: '概览',
  },
  services: {
    title: '服务管理',
    subtitle: '配置和管理备份目标位置',
    addService: '添加服务',
    local: '本地目录',
    ndn: 'NDN网络',
    online: '在线',
    offline: '离线',
    testConnection: '测试连接',
    storageUsed: '存储使用',
    compression: '压缩',
    encryption: '加密',
    enabled: '已启用',
    disabled: '已禁用',
    serviceName: '服务名称',
    serviceType: '服务类型',
    storageConfig: '存储配置',
    serverConfig: '服务器配置',
    storagePath: '存储路径',
    serverUrl: '服务器地址',
    username: '用户名',
    password: '密码',
    accessKey: '访问密钥',
    secretKey: '密钥',
    bucket: '存储桶',
    region: '区域',
    endpoint: '端点',
  },
  tasks: {
    title: '任务列表',
    subtitle: '监控所有备份和恢复任务',
    running: '执行中',
    completed: '已完成',
    paused: '已暂停',
    failed: '已失败',
    queued: '等待中',
    backup: '备份',
    restore: '恢复',
    pause: '暂停',
    resume: '继续',
    stop: '停止',
    progress: '进度',
    speed: '速度',
    remaining: '剩余时间',
    allTasks: '全部任务',
    runningTasks: '执行中',
    filterTasks: '筛选任务',
    searchPlaceholder: '搜索任务名称或计划',
    planName: '计划名称',
    taskType: '任务类型',
    startTime: '开始时间',
    endTime: '结束时间',
    duration: '耗时',
    dataSize: '数据大小',
    createRestore: '创建恢复',
    restoreTitle: '创建恢复任务',
    restoreSubtitle: '从备份中恢复文件',
    selectBackup: '选择备份',
    restoreTarget: '恢复目标',
    selectTarget: '选择目标',
    originalLocation: '原始位置',
    customLocation: '自定义位置',
    overwriteFiles: '覆盖文件',
    skipExisting: '跳过已存在',
  },
  settings: {
    title: '设置',
    subtitle: '配置系统首选项和高级选项',
    general: '常规',
    notifications: '通知',
    security: '安全',
    performance: '性能',
    advanced: '高级',
    language: '界面语言',
    timezone: '时区',
    autoStart: '启动时自动运行',
    minimizeToTray: '最小化到系统托盘',
    autoUpdate: '自动检查更新',
    lightMode: '亮色模式',
    darkMode: '暗色模式',
  },
};

const enTranslations: Translations = {
  common: {
    save: 'Save',
    cancel: 'Cancel',
    delete: 'Delete',
    edit: 'Edit',
    add: 'Add',
    search: 'Search',
    filter: 'Filter',
    all: 'All',
    status: 'Status',
    type: 'Type',
    name: 'Name',
    actions: 'Actions',
    details: 'Details',
    back: 'Back',
    next: 'Next',
    previous: 'Previous',
    finish: 'Finish',
    loading: 'Loading',
    error: 'Error',
    success: 'Success',
    warning: 'Warning',
    info: 'Info',
    create: 'Create',
    run: 'Run',
    logs: 'Logs',
    restore: 'Restore',
    connection: 'Connection',
    test: 'Test',
    enabled: 'Enabled',
    disabled: 'Disabled',
    required: 'Required',
    optional: 'Optional',
    today: 'Today',
    tomorrow: 'Tomorrow',
    minutes: 'minutes',
    hours: 'hours',
    days: 'days',
    weeks: 'weeks',
    page: 'Page',
    of: 'of',
    showing: 'Showing',
    results: 'results',
  },
  nav: {
    dashboard: 'Dashboard',
    plans: 'Backup Plans',
    services: 'Services',
    tasks: 'Tasks',
    settings: 'Settings',
  },
  dashboard: {
    title: 'Dashboard',
    subtitle: 'System overview and quick actions',
    activeTasks: 'Active Tasks',
    totalBackupSize: 'Total Backup Size',
    backupPlans: 'Backup Plans',
    successRate: 'Success Rate',
    currentTasks: 'Current Tasks',
    recentActivities: 'Recent Activities',
    backupServices: 'Backup Services',
    createNewPlan: 'Create New Plan',
    backupNow: 'Backup Now',
    addService: 'Add Service',
    viewAll: 'View All',
  },
  plans: {
    title: 'Backup Plans',
    subtitle: 'Manage and configure automatic backup plans',
    createNew: 'Create New Plan',
    enabled: 'Enabled',
    disabled: 'Disabled',
    source: 'Source',
    destination: 'Destination',
    schedule: 'Schedule',
    nextRun: 'Next Run',
    lastRun: 'Last Run',
    runNow: 'Run Now',
    viewLogs: 'View Logs',
    healthy: 'Healthy',
    warning: 'Warning',
    triggerType: 'Trigger Type',
    scheduled: 'Scheduled',
    eventTriggered: 'Event Triggered',
    manualOnly: 'Manual Only',
    schedulePeriod: 'Schedule Period',
    daily: 'Daily',
    weekly: 'Weekly',
    monthly: 'Monthly',
    eventDelay: 'Event Delay',
    planName: 'Plan Name',
    planDescription: 'Plan Description',
    selectDirectories: 'Select Directories',
    selectService: 'Select Service',
    backupType: 'Backup Type',
    fullBackup: 'Full Backup',
    incrementalBackup: 'Incremental Backup',
    versionsToKeep: 'Versions to Keep',
    priority: 'Priority',
    high: 'High',
    medium: 'Medium',
    low: 'Low',
    basicInfo: 'Basic Info',
    directories: 'Directories',
    service: 'Service',
    trigger: 'Trigger',
    advanced: 'Advanced',
    review: 'Review',
    overview: 'Overview',
  },
  services: {
    title: 'Service Management',
    subtitle: 'Configure and manage backup destinations',
    addService: 'Add Service',
    local: 'Local Directory',
    ndn: 'NDN Network',
    online: 'Online',
    offline: 'Offline',
    testConnection: 'Test Connection',
    storageUsed: 'Storage Used',
    compression: 'Compression',
    encryption: 'Encryption',
    enabled: 'Enabled',
    disabled: 'Disabled',
    serviceName: 'Service Name',
    serviceType: 'Service Type',
    storageConfig: 'Storage Configuration',
    serverConfig: 'Server Configuration',
    storagePath: 'Storage Path',
    serverUrl: 'Server URL',
    username: 'Username',
    password: 'Password',
    accessKey: 'Access Key',
    secretKey: 'Secret Key',
    bucket: 'Bucket',
    region: 'Region',
    endpoint: 'Endpoint',
  },
  tasks: {
    title: 'Task List',
    subtitle: 'Monitor all backup and restore tasks',
    running: 'Running',
    completed: 'Completed',
    paused: 'Paused',
    failed: 'Failed',
    queued: 'Queued',
    backup: 'Backup',
    restore: 'Restore',
    pause: 'Pause',
    resume: 'Resume',
    stop: 'Stop',
    progress: 'Progress',
    speed: 'Speed',
    remaining: 'Remaining',
    allTasks: 'All Tasks',
    runningTasks: 'Running',
    filterTasks: 'Filter Tasks',
    searchPlaceholder: 'Search tasks or plans',
    planName: 'Plan Name',
    taskType: 'Task Type',
    startTime: 'Start Time',
    endTime: 'End Time',
    duration: 'Duration',
    dataSize: 'Data Size',
    createRestore: 'Create Restore',
    restoreTitle: 'Create Restore Task',
    restoreSubtitle: 'Restore files from backup',
    selectBackup: 'Select Backup',
    restoreTarget: 'Restore Target',
    selectTarget: 'Select Target',
    originalLocation: 'Original Location',
    customLocation: 'Custom Location',
    overwriteFiles: 'Overwrite Files',
    skipExisting: 'Skip Existing',
  },
  settings: {
    title: 'Settings',
    subtitle: 'Configure system preferences and advanced options',
    general: 'General',
    notifications: 'Notifications',
    security: 'Security',
    performance: 'Performance',
    advanced: 'Advanced',
    language: 'Language',
    timezone: 'Timezone',
    autoStart: 'Auto start on boot',
    minimizeToTray: 'Minimize to tray',
    autoUpdate: 'Auto check updates',
    lightMode: 'Light Mode',
    darkMode: 'Dark Mode',
  },
};

export const translations = {
  'zh-cn': zhTranslations,
  'en': enTranslations,
};

export type Language = keyof typeof translations;