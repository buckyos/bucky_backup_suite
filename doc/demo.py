
import threading
import logging

# souce和target的匹配，主要是source向target匹配
# target有两种，1种是保存chunklist的，1种是有onlien folder概念的(WebDAV)
# source本身可以有很多种，但其只需要知道当前task配置的target是什么类型，就可以构建匹配的local scan逻辑
# 从复杂度来说，最复杂的情况是 source构建的folder,target也支持的是folder,这个就是CYFS Drive的状态
# 最简单的情况，是Source构建Thunk list,target也是支持chunk list的，这个是最简单的
# Engine主要提供Task的状态管理、CheckPoint的状态管理、Chunk->Chunk,(Folder->Chunk?) Chunk->Folder,Folder->Folder 的标准驱动逻辑，并提供基础的Diff算法

# CheckPoint的类选择由Target决定,Target一般会保存CheckPoint的信息
class ChunkListCheckPoint:
# ChunkListCheckPoint的存储设计可以比较简单的用全内存的方法
# 构建Chunk(本地打包) 一般需要消耗临时空间，所以其运行模式通常倾向于 构建1个Chunk+上传一个Chunk，磁盘上最多保存2个Chunk就够了
    def __init__(self):
        self.last_checkpoint_id = 0
        self.check_point_id = 0
        self.chunklist = []
        self.version = 0


class FolderCheckPoint:
# FolderCheckPoint的存储设计
# 1. ObjectTable@sqlite,通过遍历subfiles和subdir,列出所有的文件和目录，不用json保存是因为常见的DOM解析会消耗大量的资源
# 2. checkpoint.meta@json,保存了一些类似区块头的信息，比如root_tree_hash,version等。该信息存在说明该CheckPoint对应的数据扫描已经完成了
# 3. backup source经常通过扫描本地磁盘来构建ObjectTable,通过创建checkpoint.meta来说明扫描完成了
# 4. 计算文件Hash和Diff的工作是系统性能优化的关键，什么时候做，由谁做应该在一开始就进行仔细的设计 
    def __init__(self):
        self.last_checkpoint_id = 0
        self.filelist = {}
        self.version = 0

class BackupSource:
    def _scan_dir(self,dir,task_info,task_checkpoint_db):
        while task_info.state == "running":
            for sub_file in dir:
                pass

            for sub_dir in dir:
                self._scan_dir(sub_dir,task_info,task_checkpoint_db) 


# 在本伪代码中，带有await的都是潜在的网络通信函数
class BackupEngine:
    def __init__(self):
        self.task_mgr = {}
    
    # 关键入口函数，从这里开始
    async def create_backup_task(self, source_config, target_config):
        target = create_target_by_config(target_config)
        source = create_source_by_config(source_config)
        target_mode = target.get_mode()
        source_mode = source.get_mode()

        if target_mode == "chunklist" and source_mode == "chunklist":
            task_mode = "chunklist" # 这是最简单的模式
        elif target_mode == "folder" and source_mode == "folder":
            task_mode = "folder" # 这是过去CYFSDrive的模式，最复杂
        elif target_mode == "folder" and source_mode == "chunklist":
            task_mode = "chunk2folder" # 在Target上，有可能用标准的文件流量方式看到备份记录 
        elif target_mode == "chunklist" and source_mode == "folder":
            task_mode = "folder2chunk" # 这个模式就是现在备份到DMC的模式
        else:
            raise Exception("Not supported mode")
        
        target_support_diff_list = target.support_diff()
        source_support_diff_list = source.support_diff()
        diff_mode = get_diff_mode(target_support_diff_list,source_support_diff_list)
        if diff_mode == None:
            logging.warn("没有可用的增量算法，启用全量备份")

        # 待备份实体id,这个id代表一个可持续的，增量备份实体,一般可以用 用户名:设备名:目录名表达，
        # 这个目录名是逻辑意义的，比如待备份的本地文件夹移动后，source可以维持该实体id不变
        source_entitiy = source.get_entity()
        # 锁定一下source_entitiy的状态，确保在source_finish之前,entity不会变化
        source.lock_entity()
        # 获得target上持有的，该entiry的最后一个完成的check point
        last_check_point_id = await target.get_last_check_point(source_entitiy)
        
        # 创建新的（未完成）任务并保存在task_mgr_db中，根据原理，同一个source_entitiy+target只能有一个未完成的任务
        (is_exist,backup_task_id) = self.task_mgr.create_backup_task(source, target,task_mode, source_entitiy,last_check_point_id,diff_mode)
        if is_exist is False:
            # 在target上创建新的check point,此时该source_entitiy在target看来，有一个新的，未完成的check_point
            await target.create_new_check_point(source_entitiy,task_mode,last_check_point_id+1)
    
        return (is_exist,backup_task_id)

    # 让一个任务恢复执行，备份任务可能很长，会碰到各种异常，因此BackupEngine要做好task的状态管理
    def resume_backup_task(self, backup_task_id):
        task_info = self.task_mgr.get_task_info(backup_task_id)
        if task_info == None:
            raise Exception("Task not found")
        
        if task_info.state == "paused":
            task_info.state = "running"
        else:
            return False # Task is not paused, cannot start
        
        # Task的基本工作模式还是生产者消费：source生产，target消费
        # source通过扫描source_entitiy构建target可以保存的backup_item，随后target扫描未备份的backup_item并根据其自有逻辑完成备份
        # source完成所有扫描后，会最后确定check_point的一些关键信息，比如root_tree_hash，可以帮助target对整个任务进行有效校验
         
        # 启动source主导的工作线程
        source_thread = threading.Thread(target=source_local_main, args=(task_info))
        source_thread.start()
        # 启动target主导的工作线程
        target_thread = threading.Thread(target=target_transfor_main, args=(task_info))
        target_thread.start()
        return True

    
    def pause_backup_task(self, backup_task_id):
        task_info = self.task_mgr.get_task_info(backup_task_id)
        if task_info == None:
            raise Exception("Task not found")
        if task_info.state == "running":
            task_info.state = "paused"
        else:
            return False
        return True

    def delete_backup_task(self, backup_task_id):
        pass

    def get_backup_task_status(self, backup_task_id):
        task_info = self.task_mgr.get_task_info(backup_task_id)
        if task_info == None:
            raise Exception("Task not found")
        return task_info.state
    
    def chunk_source_local_main(self, task_info):
        task_checkpoint_db = load_checkpoint(task_info.source_entitiy,task_info.checkpoint_uuid)
        source = task_info.source
        while task_info.state == "running":
            #按时间排序的未传输chunk list
            pending_transfor_chunklist = task_checkpoint_db.get_pending_transfor_chunklist()
            if pending_transfor_chunklist.total_chunk_size() >= self.max_pending_size:
                sleep(1)
                continue
            # source尝试在本地磁盘上构建一个新的chunk,注意该过程的细节engine不关心，如果中断后可能要重新构建
            # source在构建chunk的过程中可以通过理解上一个checkpoint的数据，来构建更少的数据
            new_chunk,is_finish = source.generate_chunk(task_info,task_info.support_chunk_size_list,pending_transfor_chunklist)
            # 构建的chunk可以是一个新的chunk，也可以是一个增量chunk
            logging.info(new_chunk.size,new_chunk.hash,new_chunk.diff_info)
            task_checkpoint_db.push_pending_chunk(new_chunk)
            if is_finish:
                # 整个task的总大小和chunklist已经构建完成,source的工作结束了
                task_checkpoint_db.source_finish() 
                break

    def chunk_target_transfor_main(self, task_info):
        task_checkpoint_db = load_checkpoint(task_info.source_entitiy,task_info.checkpoint_uuid)
        target = task_info.target
        while task_info.state == "running":
            pending_transfor_chunklist = task_checkpoint_db.get_pending_transfor_chunklist()
            chunk = pending_transfor_chunklist.pop_chunk()
            if chunk == None:
                if task_checkpoint_db.is_source_finish():
                    await target.finish_check_point(task_info.checkpoint_uuid)
                    task_checkpoint_db.finish()
                    task_info.state = "finish"
                    break
                else:
                    sleep(1)
                    continue
            
            if chunk.diff_info != None:
                # 在系统配置支持时，这里可以统一由engine来计算diff并上传
                diff_chunk = await target.calculate_diff_and_upload(chunk)
                task_checkpoint_db.replace_chunk(chunk,diff_chunk)
            else:
                # target内部可以做断点续传的优化，engine可以为这个断点续传提供一些基本的支持
                # target.upload_chunk内部实现我们鼓励使用标准的Put Chunka和 Patch Chunk协议，这样服务器可以进一步的通过协议优化减少网络流量
                await target.upload_chunk(chunk)
                task_checkpoint_db.finish_chunk(chunk)


           

    def folder_source_local_main(self,task_info):
        task_checkpoint_db = load_checkpoint(task_info.source_entitiy,task_info.checkpoint_uuid)
        source = task_info.source
        while task_info.state == "running":
            if task_checkpoint_db.is_finish_scan() == False:
                # source扫描本地目录,这个过程中task_checkpoint_db会不同增加pending的文件和目录
                # source可以根据自己的逻辑，来决定怎么扫描,扫描的过程中要充分使用last_checkpoint db来提高性能
                is_finish = self.source._scan_dir(task_info.get_root_dir(),task_info,task_checkpoint_db)
                if is_finish: #可能中途中断了
                    task_checkpoint_db.finish_scan()
            else:
                # 快速扫描完成后，需要完成check_point的完整构建
                # 核心目标是得到root_tree_hash，以及各个目录的tree_hash
                filelist = task_checkpoint_db.get_no_hash_files(self.TOTAL_SIZE)
                if len(filelist) > 0:
                    for file in filest:
                        if task_info.state != "running":
                            return
                        
                        hash,diff = calculate_hash_and_diff(file)
                        task_checkpoint_db.update_file_hash_and_diff(file,diff)
                else:
                    # 根据dir里的subfile的hash和subdir hash 计算hash.这里不涉及到任何diff操作
                    # 这里的计算已经不涉及到大规模的IO了
                    while task_info.state == "running":
                        dir = task_checkpoint_db.get_no_hash_dir()
                        if sub_dir != None:
                            dir_hash,dir_diff = calculate_dir_hash_and_dif(sub_dir)
                            task_checkpoint_db.update_dir_hash(sub_dir,hash,dir_diff)   

                    # 本地扫描结束
                    task_checkpoint_db.finish_source()       
                        
    def folder_target_transfor_main(self,task_info):
        task_checkpoint_db = load_checkpoint(task_info.source_entitiy,task_info.checkpoint_uuid)
        target = task_info.target
        while task_info.state == "running":
            if task_checkpoint_db.is_finish_scan() == False:
                file_list = task_checkpoint_db.get_new_files();
                for file in file_list:
                    await target.upload_file(file)
                    task_checkpoint_db.finish_file(file)
            else:
                # 等待source完成所有目录的hash计算
                if task_checkpoint_db.is_finish_source():
                    # 通过深度遍历，开始逐步上传文件
                    sub_dir,dir_diff = task_checkpoint_db.pop_unfinish_dir()
                    if sub_dir != None:
                        is_exist,filelist = await target.upload_dir(sub_dir,dir_diff)
                        if is_exist:
                            continue
                        else: 
                            if filelist != None:
                                for file_hash in filelist:
                                    file = task_checkpoint_db.get_fie_by_hash(file_hash)
                                    #在实现内部，会根据file是否为全新文件等条件这，综合决定是否要做diff
                                    await target.upload_file_and_diff(file)
                                    task_checkpoint_db.finish_file(file) #可以更新一下有效进度

                                task_checkpoint_db.finish_dir(sub_dir)
                            else:
                                subfiles = task_checkpoint_db.get_subfiles(sub_dir)    
                                for file in subfiles:
                                    await target.upload_file_and_diff(file)
                                    task_checkpoint_db.finish_file(file)
                                task_checkpoint_db.finish_dir(sub_dir)    
                    else:
                        # 所有目录上传完成
                        task_checkpoint_db.finish_target()
                        break


# target server如果能支持一些通用协议，可以进一步简化targt的实现          
# chunk target server的几个关键协议
# - CheckChunkList 给服务器发送一个ChunkList, 服务器返回其不拥有的ChunkList
# - PutChunk 给服务器发送一个Chunk,如果服务器已经拥有了改Chunk,则提前中断
# - PatchChunk 给服务器以Diff方式发送Chunk , Put NewChunkHash =  OldChunkHash + PatchDataHash + DiffOp, 服务器如果没有OldChunkHash，或不支持DiffOp则返回错误，否则客户端继续上传DiffData,上传完成后服务器会确认新的Hash

# folder target的几个关键协议
# - PubDir 给服务器发送一个DirHash,服务器返回其不拥有的sub itemlist
# - PatchDir 给服务器发送 NewDirHash = OldDirHash + PatchJson  给Dir打Patch是标准格式
# - PutFile 给服务器发送一个FileHash+ChunkList，服务器如果已经拥有了该文件，则提前中断,否则可以选择返回其确实的ChunkList
# - PatchFile 给服务器发送一个FileHash = OldFielHash + DiffData + DiffOp,服务器如果没有OldFileHash,或不支持DiffOp则返回错误，否则客户端继续上传DiffData
# - PutChunk 已经描述 
# - PatchChunk 已经描述


if __name__ == "__main__":
    engine = BackupEngine()
    backup_task = engine.create_backup_task()
    backup_task.resume()
    while backup_task.get_status() == "running":
        print(backup_task.get_status()) # 可以展示进度
