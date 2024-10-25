import threading
import logging

def rpc_call(url: str, method: str, params: dict) -> dict:
    pass

class Engine:
    def __init__(self):
        self.next_source_id = 1
        self.next_target_id = 1
        self.sources = {}
        self.targets = {}

    def register_source(self, source_url: str) -> int:
        source_id = self.next_source_id
        self.sources[source_id] = Source(source_url)
        self.next_source_id = self.next_source_id + 1
        return source_id


    def register_target(self, target_url: str):
        target_id = self.next_target_id
        self.targets[target_id] = Target(target_url)
        self.next_target_id = self.next_target_id + 1
        return target_id

    def create_task(self, 
        source_id: int,
        source_param: str,
        target_id: int,
        target_param: str) -> 'Task': # type: ignore
        target = self.targets[target_id]
        source = self.sources[source_id]
        target_accept_types = target.accept_types()
        source_output_types = source.output_types()
        data_type = target_accept_types.intersect(source_output_types)
        task = Task(source, source_param, target, target_param, data_type)
        return task
    
class Task:

    def __init__(self, source: 'Source', source_param: str, target: 'Target', target_param: str):
        self.source = source
        self.source_param = source_param
        self.source_task = source.create_source_task(source_param)
        self.target = target
        self.target_param = target_param
        self.target_task = target.create_target_task(target_param)
        self.source_states = {}
        self.next_source_state_id = 1
        self.locked_source_state_id = None
        self.checkpoints = {}
        self.next_checkpoint_version = 1

    def lock_source(self):
        # should unlock it before lock
        if self.locked_source_state_id:
            self.unlock_source()

        # save the original state of source before backup
        original_state = self.source.original_state()
        self.locked_source_state_id = self.next_source_state_id
        self.next_source_state_id = self.next_source_state_id + 1

        # lock the source state(eg: snapshort, copy, move...)

        locked_state = self.source.lock_state(original_state)
        self.source_states[self.locked_source_state_id] = {original_state, locked_state}

    # unlock the state of source to that locked before backup. (eg: delete the snapshot or clear the copied datas)
    def unlock_source(self):
        if self.locked_source_state_id is None:
            return

        locked_state = self.source_states[self.locked_source_state_id]
        self.source.unlock_state(self.source_param, locked_state.original_state)
        del self.source_states[self.locked_source_state_id]
        self.locked_source_state_id = None

    def create_checkpoint(self, is_delta: bool) -> 'Checkpoint':
        last_checkpoint = find_last_checkpoint()
        if not last_checkpoint.is_finish():
            # you should abort the last checkpoint or wait it finish
            return

        version = self.next_checkpoint_version
        prev_version = None
        if is_delta:
            prev_version = last_checkpoint.version
        checkpoint = Checkpoint(self.locked_source_state_id, prev_version, self)
        self.next_checkpoint_version = self.next_checkpoint_version + 1

        self.checkpoints[version] = checkpoint
        return checkpoint


class StorageReader:
    def read_dir(self, path: str) -> list:
        # return the list of sub-dir and file in the directory specified by path
        pass

    def file_size(self, path: str) -> int:
        # return the size of the file specified by path
        pass

    def read_file(self, path: str, offset: int, length: int) -> bytes:
        # return the content of the file specified by path, start from offset, length bytes
        pass
    

    def read_link(self, path: str) -> str:
        # return the target of the link specified by path
        pass

    def stat(self, path: str) -> 'StorageItemAttributes':
        # return the attributes of the item specified by path
        pass



class Source:
    def __init__(self, source_url: str):
        self.source_url = source_url

    def create_source_task(self, source_param: str) -> 'SourceTask':
        return SourceTask(self, source_param)
    

class SourceTask:
    def __init__(self, source: Source, source_param: str):
        self.source = source
        self.source_param = source_param

    def original_state(self) -> str:
        rpc_call(self.source.source_url, 'original_state', {'source_param': self.source_param})


    def lock_state(self, original_state: str) -> str:
        rpc_call(self.source.source_url, 'lock_state', {'source_param': self.source_param, 'original_state': original_state})

    def restore_state(self, original_state: str):
        rpc_call(self.source.source_url, 'restore_state', {'source_param': self.source_param, 'original_state': original_state})

    def source_locked(self, locked_state_id: int, locked_state: str) -> 'Sourcelocked':
        return Sourcelocked(self, locked_state_id, locked_state)

class Sourcelocked(StorageReader):
    def __init__(self, source_task: 'SourceTask', locked_state_id: int, locked_state: str):
        self.source_task = source_task
        self.locked_state_id = locked_state_id
        self.locked_state = locked_state

    def prepare(self):
        rpc_call(self.source.source_url, 'prepare', {'source_param': self.source_param})

class Target:

    def __init__(self, target_url: str):
        self.target_url = target_url

    def create_target_task(self, target_param: str) -> 'TargetTask':
        return TargetTask(self, target_param)

class TargetTask:
    def __init__(self, target: Target, target_param: str):
        self.target = target
        self.target_param = target_param

    def target_checkpoint(self) -> 'TargetCheckPoint':
        rpc_call(self.target.target_url, 'target_checkpoint', {'target_param': self.target_param})

class TargetCheckPoint(StorageReader):

    def __init__(self, target_task: 'TargetTask', target_param: str):
        self.target_task = target_task
        self.target_param = target_param

    def transfer(self):
        rpc_call(self.target.target_url, 'transfer', {'target_param': self.target_param})

STATUS_STANDBY = 0
STATUS_PREPARING = 1
STATUS_PREPARE_STARTED = 2
STATUS_STARTING = 3
STATUS_SOURCE_STARTED = 4
STATUS_START = 5
STATUS_STOPPING = 6
STATUS_SOURCE_STOPPED = 7
STATUS_TARGET_STOPPED = 8
STATUS_STOPPED = 9
STATUS_SUCCESS = 10
STATUS_FAILED = 11

class Checkpoint(StorageReader):

    def __init__(self, locked_state_id: int, prev_version: int | None, task: 'Task'):
        self.locked_state_id = locked_state_id
        self.prev_version = prev_version
        self.task = task
        self.target_checkpoint = None
        self.is_preparing_source = False
        self.is_transfer = False
        self.source_locked = None
        self.status = STATUS_STANDBY

    def prepare_source(self):
        if self.status == STATUS_STANDBY or self.status == STATUS_STOPPED or self.status == STATUS_FAILED:
            self.status = STATUS_PREPARING
            self.prepare_source_without_status()

    def prepare_source_without_status(self):
        locked_state = self.task.source_states[self.locked_state_id].locked_state;
        self.source_locked = self.task.source_task.source_locked(self.locked_state_id, locked_state)
        is_success = self.source_locked.prepare()
        if is_success:
            if self.status == STATUS_PREPARING:
                self.status = STATUS_PREPARE_STARTED
            elif self.status == STATUS_STARTING:
                self.status = STATUS_SOURCE_STARTED
        else:
            self.status = STATUS_FAILED
            # stop target

    def transfer(self, is_compress: bool):
        if self.status == STATUS_STANDBY or self.status == STATUS_STOPPED or self.status == STATUS_FAILED:
            self.status = STATUS_STARTING
            self.prepare_source_without_status()
        elif self.status == STATUS_PREPARING:
            self.status = STATUS_STARTING
        elif self.status == STATUS_PREPARE_STARTED:
            self.status = STATUS_SOURCE_STARTED
        elif self.status == STATUS_STARTING or self.status == STATUS_SOURCE_STARTED or self.status == STATUS_START:
            return "pending"
        elif self.status == STATUS_STOPPING or self.status == STATUS_SOURCE_STOPPED or self.status == STATUS_TARGET_STOPPED:
            return "invalid-status"
        elif self.status == STATUS_SUCCESS:
            return "ok"

        self.wait_status([STATUS_SOURCE_STARTED, STATUS_FAILED])

        target_checkpoint = self.task.target_task.target_checkpoint()
        target_checkpoint.transfer()
        self.target_checkpoint = target_checkpoint

    def wait_status(self, status: list[int]):
        while not self.status in status:
            sleep(1)


    def next_chunk(self, capacities: list[int]):
        files = files_db.list_unpack_files()
        if not files:
            if self.source_locked.is_files_scan_finish():
                return None
            else:
                self.source_locked.wait_new_file();
                files = files_db.list_unpack_files()

        
        chunk = chunk_db.add_new_chunk()
        return chunk

    def stop(self):
        self.status = STATUS_STOPPING
        self.source_locked.stop()
        self.status = STATUS_SOURCE_STOPPED
        self.target_checkpoint.stop()
        self.status = STATUS_TARGET_STOPPED
        self.status = STATUS_STOPPED
        # wait stop

class Chunk:
    def __init__(self, checkpoint: 'Checkpoint', is_compress: bool):
        self.checkpoint = checkpoint
        self.is_compress = is_compress
        pass

    def len(self) -> int: # the real length will be less in compress
        pass

    def read(self, offset: int, len: int) -> bytes:
        if self.checkpoint.status == STATUS_SUCCESS:
            self.read_from_source(offset, len)
        else:
            self.read_from_target(offset, len)

    def read_from_source(self, offset: int, len: int) -> bytes:
        is_new_block = False
        file_block = self.file_block_at(offset)
        if not file_block:
            # need new block
            is_new_block = True
            files = files_db.list_unpack_files()
            if not files:
                if self.capacity - self.real_len() < FREE_LIMIT or # is full
                    self.file_scan_finish(): # the last chunk will be not full
                    chunk_db.set_finish()
                    return None
                else:
                    wait_new_files()
                    files = files_db.list_unpack_files()
            
            file = self.select_file(files)
            file_block = file

            if self.checkpoint.is_delta:
                if not file.is_diff():
                    file_diff = files_db.find_diff(file)
                    if not file_diff:
                        last_version_file = self.checkpoint.get_file_in_last_version(file)
                        file_diff = diff(last_version_file, file_block.file)
                        files_db.add_file_diff(file_diff)
                        file_block = file_diff
                else:
                    # calc diff by source
                    pass

        if self.is_compress:
            # compress it when read
            file_block = compress(file_block)

        if is_new_block:
            chunk_db.add_file_block(file_block)

        if self.capacity - self.real_len() < FREE_LIMIT or # is full
            (self.file_scan_finish() and self.is_last_one()): # the last chunk will be not full
            chunk_db.set_finish()

        return file_block.content
        

    def read_from_target(self, offset: int, len: int) -> bytes:
        pass