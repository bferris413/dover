#!/bin/bash

cargo watch --watch-when-idle -x 'run -- files test_files/tuple_struct.rs test_files/tuple_struct_diff.rs > ast.out 2>&1'
#cargo watch --watch-when-idle -x 'run -- overview test_files/tuple_struct.rs > ast.out 2>&1'
#cargo watch --watch-when-idle -x 'run -- overview src/lib.rs > ast.out 2>&1'
#cargo watch --watch-when-idle -x 'run -- files src/lib.rs test_files/lib_copy.rs > ast.out 2>&1'
#cargo watch --watch-when-idle -x 'test git -- --nocapture > ast.out 2>&1'
#cargo watch --watch-when-idle -x 'run > ast.out 2>&1'
