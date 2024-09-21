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
        task = Task(self.sources[source_id], source_param, self.targets[target_id], target_param)
        return task
    
class Task:

    def __init__(self, source: 'Source', source_param: str, target: 'Target', target_param: str):
        self.source = source
        self.source_param = source_param
        self.source_task = source.create_source_task(source_param)
        self.target = target
        self.target_param = target_param
        self.target_task = target.create_target_task(target_param)
        self.preserved_states = {}
        self.next_preserved_state_id = 1
        self.checkpoints = {}
        self.next_checkpoint_version = 1

    def preserve_source(self) -> int:
        # save the original state of source before backup
        original_state = self.source.original_state()
        preserved_state_id = self.next_preserved_state_id
        self.next_preserved_state_id = self.next_preserved_state_id + 1

        # make the source preserved(eg: snapshort, read-only...)

        preserved_state = self.source.preserved_state(original_state)
        self.preserved_states[preserved_state_id] = {original_state, preserved_state}

        return preserved_state_id

    # restore the state of source to that preserved before backup. (eg: delete the snapshot or restore the files to writable)
    def restore_source(self, preserved_state_id: int):
        if preserved_state_id not in self.preserved_states:
            return

        preserved_state = self.preserved_states[preserved_state_id]
        self.source.restore_state(self.source_param, preserved_state.original_state)
        del self.preserved_states[preserved_state_id]

    def prepare_checkpoint(self, preserved_state_id: int, is_delta: bool) -> 'Checkpoint':
        preserved_state = self.preserved_states[preserved_state_id].preserved_state;
        source_preserved = self.source_task.source_preserved(preserved_state_id, preserved_state)

        def read_meta_from_storage_reader(reader: 'StorageReader') -> 'CheckpointMeta':
            # todo read each directory and file's meta from reader, and return the meta of checkpoint
            pass

        def delta_meta_from_prev_checkpoint(current_meta: 'CheckpointMeta', prev_checkpoint_meta: 'CheckpointMeta') -> 'CheckpointMeta':
            # todo calculate the delta meta from prev_checkpoint_meta, and return the delta meta of checkpoint
            pass

        def find_last_finish_checkpoint() -> 'Checkpoint':
            # todo find the last finish checkpoint from source_preserved
            pass

        current_meta = read_meta_from_storage_reader(source_preserved)


        if is_delta:
            last_checkpoint = find_last_finish_checkpoint()
            last_checkpoint_meta = read_meta_from_storage_reader(last_checkpoint)
            current_meta = delta_meta_from_prev_checkpoint(current_meta, last_checkpoint_meta)

        current_meta.version = self.next_checkpoint_version
        checkpoint = Checkpoint(current_meta, preserved_state_id, last_checkpoint_meta.version if last_checkpoint_meta else None, self)
        self.next_checkpoint_version = self.next_checkpoint_version + 1

        self.checkpoints[current_meta.version] = checkpoint
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
        rpc_call(self.source.source_url, 'get_original_state', {'source_param': self.source_param})


    def preserved_state(self, original_state: str) -> str:
        rpc_call(self.source.source_url, 'get_preserved_state', {'source_param': self.source_param, 'original_state': original_state})

    def restore_state(self, original_state: str):
        rpc_call(self.source.source_url, 'restore_state', {'source_param': self.source_param, 'original_state': original_state})

    def source_preserved(self, preserved_state_id: int, preserved_state: str) -> 'SourcePreserved':
        return SourcePreserved(self, preserved_state_id, preserved_state)

class SourcePreserved(StorageReader):
    def __init__(self, source_task: 'SourceTask', preserved_state_id: int, preserved_state: str):
        self.source_task = source_task
        self.preserved_state_id = preserved_state_id
        self.preserved_state = preserved_state

class Target:

    def __init__(self, target_url: str):
        self.target_url = target_url

    def create_target_task(self, target_param: str) -> 'TargetTask':
        return TargetTask(self, target_param)

class TargetTask:
    def __init__(self, target: Target, target_param: str):
        self.target = target
        self.target_param = target_param

    def fill_target_meta(self, meta: 'CheckpointMeta') -> [str]:
        [filled_meta, target_meta] = rpc_call(self.target.target_url, 'fill_target_meta', {'target_param': self.target_param, 'meta': meta})
        meta = filled_meta # fill meta for each item(dir, file, link...)
        return target_meta # return some info for target, and set it to target service when transfer begin
    
    def target_checkpoint_from_filled_meta(self, meta: 'CheckpointMeta', target_meta: [str]) -> 'TargetCheckPoint':
        return TargetCheckPoint(self, self.target_param, target_meta)

class TargetCheckPoint(StorageReader):

    def __init__(self, target_task: 'TargetTask', target_param: str, meta: 'CheckpointMeta', target_meta: [str]):
        self.target_task = target_task
        self.target_param = target_param
        self.meta = meta
        self.target_meta = target_meta

    def transfer(self):
        rpc_call(self.target.target_url, 'transfer', {'target_param': self.target_param, 'meta': self.meta, 'target_meta': self.target_meta})

class Checkpoint(StorageReader):

    def __init__(self, meta: 'CheckpointMeta', preserved_state_id: int, prev_version: int | None, task: 'Task'):
        self.meta = meta
        self.preserved_state_id = preserved_state_id
        self.prev_version = prev_version
        self.task = task
        self.target_meta = None

    def transfer(self):
        if self.target_meta is None:
            # is first transfer
            meta = self.meta
            target_meta = self.task.target_task.fill_target_meta(meta);
            self.meta = meta # update meta to the latest
            self.target_meta = target_meta
        else:
            # is resume
            pass

        target_checkpoint = self.task.target_task.target_checkpoint_from_filled_meta(self.meta, self.target_meta)
        target_checkpoint.transfer()


class CheckpointMeta:

    def __init__(self, ):
        pass
