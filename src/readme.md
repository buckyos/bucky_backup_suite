## 核心概念
BackupPlan 备份计划，一个备份计划包含一个或多个备份任务
BackupCheckpoint 备份点，一个备份点包含一个或多个备份任务
BackupTask 备份任务，一个备份任务包含一个或多个备份项
RestoreTask 恢复任务，一个恢复任务包含一个或多个恢复项

BackupItem 备份项，一个备份项对应一个文件或目录
Chunk 数据块，一个数据块是备份任务中的最小数据单元，一个备份项可以拆分成一个或多个数据块
File 文件，一个文件是备份任务中的最小数据单元，一个备份项可以拆分成一个或多个文件
Directory 目录，一个目录是备份任务中的最小数据单元，一个备份项可以拆分成一个或多个目

BackupSourceProvider 备份源，一个备份源对应一个数据源（可以是本地的也可以是remote的）
BackupTargetProvider 备份目标，一个备份目标对应一个保存备份数据的可用存储空间（通常是remote的），



## 工程目录结构

1. backup_suite 是核心，负责管理配置，备份任务，以及提供备份和恢复的接口




