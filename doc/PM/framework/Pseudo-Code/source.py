import threading

# a demo local directory backup source

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
    def __init__(self):
        pass

    def create_source_task(self, source_param: str) -> 'SourceTask':
        # for local directory backup, source_param is the path of the directory
        # for other source, source_param may be some metadata for the specific protocol
        # eg: the metadata store in the sectors of `DMC` should be provided by developer.
        #     so the developer should get the metadata from engine before the user restore a checkpoint from 'DMC'
        return SourceTask(self, source_param)
    


class SourceTask:
    def __init__(self, path: str):
        self.path = path
        self.is_snapshot = true

    def original_state(self) -> str:
        if self.is_snapshot:
            # generate a snapshot id
            # snapshot_id = generate_snapshot_id(self.path)
            return 'snapshot:1'
        else:
            # read all attributes of the items in the directory specified by self.path

            # return a json string
            return 'readonly:{original_state for each item}'


    def lock_state(self, original_state: str) -> str:
        if self.is_snapshot:
            # create a new snapshot with name in 'original_state', and return the path of the snapshot
            return 'path of the snapshot'
        else:
            # set all items to readonly, and return the path of the root directory
            return 'path of the root directory'

    def restore_state(self, original_state: str):
        if self.is_snapshot:
            # remove the snapshot specified by original_state
            pass
        else:
            # restore permissions of the items to the state in original_state
            pass

    def source_locked(self, locked_state_id: int, locked_state: str) -> 'SourceLocked':
        # locked_state is the root directory that will be backupped
        return Sourcelocked(self, locked_state_id, locked_state)

class Sourcelocked(StorageReader):
    def __init__(self, source_task: 'SourceTask', locked_state_id: int, root_path: str):
        self.source_task = source_task
        self.locked_state_id = locked_state_id
        self.root_path = root_path

    def read_dir(self, path: str) -> list:
        # return the list of sub-dir and file in the directory specified by path
        full_path = os.path.join(self.root_path, path)
        # find all children of the directory specified by full_path
        []

    def file_size(self, path: str) -> int:
        # return the size of the file specified by path
        full_path = os.path.join(self.root_path, path)
        # get the size of the file specified by full_path
        0

    def read_file(self, path: str, offset: int, length: int) -> bytes:
        # return the content of the file specified by path, start from offset, length bytes
        full_path = os.path.join(self.root_path, path)
        # read the content of the file specified by full_path, start from offset, length bytes
        b''
    


    def read_link(self, path: str) -> str:
        # return the target of the link specified by path
        full_path = os.path.join(self.root_path, path)
        # get the target of the link specified by full_path
        # and change the target path to relative path if it's contained in self.root_path
        ''


    def stat(self, path: str) -> 'StorageItemAttributes':
        # return the attributes of the item specified by path
        full_path = os.path.join(self.root_path, path)
        # get the attributes of the item specified by full_path
        pass

    def prepare(self):
        source_thread = threading.Thread(target = source_scan_file_list, args=(task_info))
        source_thread.start()


def source_scan_file_list(source_entitiy):
    for file in source_entitiy:
        # calc diff, and it can be calc by engine in default way.
        # dirs or links
        files_db.add_file(file)

    files_db.set_scan_finish()

http.start_service() # todo