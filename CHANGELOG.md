# Version 0.1.1 (2025-09-05)

## dv-wrap

### Bug Fixes

- Corrected the cache table creation SQL statement
- Dot upload not must actually upload the file

### Improvements

- Added download support

# Version 0.1.2 (2025-09-18)

## dv-wrap

### Bug Fixes

- When `dl` determines the file wasn't changed, it also should update the cache
- Corrected the overwrite logic in `sync` operation

### Improvements

- Added timeout for `dl` operation

# Version 0.1.3 (2025-09-23)

## dv-api

### Bug Fixes

- When trying to read file, don't try to create it's parent directories if they don't exist

## dv-wrap

### Bug Fixes

- While cached file has been deleted, `dl` should redownload it

### Improvements

- Added `d` (delete) flag to `sync` operation to delete files in the destination that are not present in the source.
