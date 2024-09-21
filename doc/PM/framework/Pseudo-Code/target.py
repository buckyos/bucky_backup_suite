# a simple backup target service, DMC
class Target:
    def __init__(self, private_key: str):
        self.private_key = private_key

    def create_target_task(self, target_param: str) -> 'TargetTask':
        # may be the target_param can be ignored for DMC
        # but it's useful for other targets
        #     eg: Local directory target when restore from DMC, the developer can provide the local path for target_param
        return TargetTask(self)

class TargetTask:
    def __init__(self, target: Target):
        self.target = target

    def fill_target_meta(self, meta: 'CheckpointMeta') -> [str]:
        # todo, maybe we can do something prepare for transfer:
        # 1. create tx to buy sectors, attention: the tx should be stored in engine before it's executed to avoid reentrancy.
        # 2. allocate the sectors for each item(dir, file, link...)
        # 3. return the target address for each item will be transferred to sectors:

        #     sector[0]: {file1: {offset: 0, length: 1024}, file2: {offset: 1024, length: 1024}...}
        #     sector[1]: {file3: {offset: 0, length: 1024}, file4: {offset: 1024, length: 1024}...}
        #     ...
        sectors = [];
        sector_id = 20240921
        sector_meta = {'file1': {'offset': 0, 'length': 1024, 'pos': 0}, 'file2-splited': {'offset': 0, 'length': 1024, 'pos': 1024}}
        sectors[0] = {'sector_id': sector_id, 'sector_meta': sector_meta}
        sector_meta = {'file2-splited': {'offset': 1024, 'length': 4096, 'pos': 0}, 'file3': {'offset': 1024, 'length': 1024, 'pos': 4096}} # attention: 'file2-splited' is splited into two sectors
        sectors[1] = {'sector_id': sector_id, 'sector_meta': sector_meta}
        
        return sectors
    

    def target_checkpoint_from_filled_meta(self, meta: 'CheckpointMeta', target_meta: [str]) -> 'TargetCheckPoint':
        sectors = []
        for sector_json in target_meta:
            sector = json.loads(sector_json)
            sector_id = sector['sector_id']
            sector_meta = sector['sector_meta']
            sectors.append({sector_id, sector_meta})

        return TargetCheckPoint(self, meta, sectors)


class TargetCheckPoint(StorageReader):

    def __init__(self, target_task: 'TargetTask', meta: 'CheckpointMeta', sectors: []):
        self.target_task = target_task
        self.meta = meta
        self.sectors = sectors

    def transfer(self):
        checkpoint_engine = CheckpointEngine('${engine_url}', self.meta)

        for sector in self.sectors:
            sector_buffer = []
            sector_id = sector['sector_id']

            sector_meta = sector['sector_meta']
            for file_path, file_meta in sector_meta.items():
                offset = file_meta['offset']
                length = file_meta['length']
                pos = file_meta['pos']

                file_content = checkpoint_engine.read_file(file_path, offset, length)
                sector_buffer.write(file_content, pos)

            DMC.upload_sector(sector_id, sector_buffer)
def rpc_call(url: str, method: str, params: dict) -> dict:
    pass

class CheckpointEngine:
    def __init__(self, engine_url: str, meta: 'CheckpointMeta'):
        self.engine_url = engine_url
        self.meta = meta

    def read_file(self, file_path: str, offset: int, length: int):
        params = {'file_path': file_path, 'offset': offset, 'length': length, 'task_uuid': self.meta.task_uuid, 'version': self.meta.version}
        return rpc_call(self.engine_url, 'read_file', params)



http.start_service() # todo