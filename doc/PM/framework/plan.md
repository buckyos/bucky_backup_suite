1. Define interfaces for each module,
2. Implement the basic version of full backup, initially only supporting the simplest directories and files

    - Engine,
    - Source, provide a locally prepared directory as the backup `Source`,
    - Target, provide a local directory as the backup `Target`,

    * This is an important milestone. It can achieve the simple function of copying content from one directory to another. Although the functionality is simple, it is complete and serves as a good basic logic test version; subsequent iterations will be based on this.

3. Support `Link`,
    - Hard links, soft links
    - Links within the backup directory, links outside the backup directory
4. Support incremental backup, initially without file content `Diff`,
    - Directory addition and deletion, attribute changes
    - File addition and deletion, attribute changes, content replacement, restore old versions
5. Support `DMC Target`, TODO
6. Support `DMC Source`, TODO
7. `UI` integration, TODO
8. Support file content `Diff`,
9. Export and import backup tasks,
10. Integration testing,
