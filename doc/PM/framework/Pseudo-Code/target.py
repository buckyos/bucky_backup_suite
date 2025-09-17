import threading

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

    def target_checkpoint(self, version: int) -> 'TargetCheckPoint':
        return TargetCheckPoint(self, version)

class TargetCheckPoint(StorageReader):

    def __init__(self, target_task: 'TargetTask', version: int):
        self.target_task = target_task
        self.version = version

    def transfer(self):
        chunk_thread = threading.Thread(target = upload_chunk, args={chunks_db})

def upload_chunk(chunks_db):
    checkpoint_engine = CheckpointEngine('${engine_url}', self.version)

    sector = None

    loop:
        chunk = checkpoint_engine.next_chunk()
        if not chunk:
            checkpoint_engine.on_all_chunks_upload_success()
            break
        
        if not sector:
            sector = DMC.generate_sector()
            sector.set_sector_header('something')
            sector_db.add_sector(sector)

        sector_db.add_chunk_in_sector(sector, chunk)

        dmc_chunk = sector.add_chunk(chunk)

        pos = 0
        loop:
            data_block = chunk.read(pos, 1M)
            if data_block:
                dmc_chunk.upload_block(data_block)
            else:
                dmc_chunk.close()
                break
            

def rpc_call(url: str, method: str, params: dict) -> dict:
    pass

class CheckpointEngine:
    def __init__(self, engine_url: str, meta: 'CheckpointMeta'):
        self.engine_url = engine_url
        self.meta = meta

    def next_chunk(self):
        return rpc_call(self.engine_url, 'next_chunk', params)



http.start_service() # todo