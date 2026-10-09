# Command reference

Generated from `stlive --help` (`scripts/gen-reference.sh`). Every command prints JSON unless `--text` is given.

```
Work inside a live Pharo image: eval, inspect, debug, fix, re-run.

Usage: stlive [OPTIONS] <COMMAND>

Commands:
  init       One-time setup: download Pharo 13 and build an image with the StLive server
  instances  List instances known in the state directory
  start      Start the image headless in the background
  stop       Stop the image (destructive: unsaved state and debug sessions are lost)
  status     Image identity and status
  ping       Check reachability
  eval       Evaluate Smalltalk code ('-' reads stdin). Exceptions become debug sessions
  obj        Inspect objects by reference
  find       Search the system
  class      Classes
  method     Methods
  changes    Recorded changes made through this CLI
  test       Tests
  package    Packages
  debug      Debug sessions
  image      Image persistence, info and cloning
  save       Write changes made through this CLI back to Tonel files (minimal per-method diffs)
  load       Load a Metacello baseline; reports the packages that appeared
  raw        Send a raw command: stlive raw debug.frames session=dbg-1 limit=3
  help       Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
  -V, --version                Print version
```

## `stlive init`

```
One-time setup: download Pharo 13 and build an image with the StLive server

Usage: stlive init [OPTIONS]

Options:
      --force                  Re-download and rebuild even if present
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive start`

```
Start the image headless in the background

Usage: stlive start [OPTIONS]

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --vm <VM>                Pharo VM executable (remembered per instance) [env: STLIVE_VM=]
      --image <IMAGE>          Image prepared with the StLive package (remembered per instance) [env: STLIVE_IMAGE=]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive stop`

```
Stop the image (destructive: unsaved state and debug sessions are lost)

Usage: stlive stop [OPTIONS]

Options:
      --force                  Required confirmation
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive status`

```
Image identity and status

Usage: stlive status [OPTIONS]

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive ping`

```
Check reachability

Usage: stlive ping [OPTIONS]

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive instances`

```
List instances known in the state directory

Usage: stlive instances [OPTIONS]

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive eval`

```
Evaluate Smalltalk code ('-' reads stdin). Exceptions become debug sessions

Usage: stlive eval [OPTIONS] <CODE>

Arguments:
  <CODE>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --in <RECEIVER>          Object reference used as `self`
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --timeout <TIMEOUT>      Milliseconds before the running code is interrupted into a debug session
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive obj`

```
Inspect objects by reference

Usage: stlive obj [OPTIONS] <COMMAND>

Commands:
  show     Summary with instance variables (paginated)
  items    Elements of a collection (paginated)
  text     Full text of a string (or printString of any object), paginated by characters
  var      One instance variable
  compare  Compare two objects (identity, equality, class)
  release  Forget a reference (or --all)
  help     Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive obj show`

```
Summary with instance variables (paginated)

Usage: stlive obj show [OPTIONS] <REF>

Arguments:
  <REF>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --offset <OFFSET>        
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --raw                    Also show internals of system collections
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive obj items`

```
Elements of a collection (paginated)

Usage: stlive obj items [OPTIONS] <REF>

Arguments:
  <REF>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --offset <OFFSET>        
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive obj text`

```
Full text of a string (or printString of any object), paginated by characters

Usage: stlive obj text [OPTIONS] <REF>

Arguments:
  <REF>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --offset <OFFSET>        
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive obj var`

```
One instance variable

Usage: stlive obj var [OPTIONS] <REF> <NAME>

Arguments:
  <REF>   
  <NAME>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive obj compare`

```
Compare two objects (identity, equality, class)

Usage: stlive obj compare [OPTIONS] <A> <B>

Arguments:
  <A>  
  <B>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive obj release`

```
Forget a reference (or --all)

Usage: stlive obj release [OPTIONS] [REF]

Arguments:
  [REF]  

Options:
      --all                    
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive find`

```
Search the system

Usage: stlive find [OPTIONS] <COMMAND>

Commands:
  class         
  package       
  implementors  
  senders       
  help          Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive find class`

```
Usage: stlive find class [OPTIONS] <PATTERN>

Arguments:
  <PATTERN>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive find package`

```
Usage: stlive find package [OPTIONS] <PATTERN>

Arguments:
  <PATTERN>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive find implementors`

```
Usage: stlive find implementors [OPTIONS] <SELECTOR>

Arguments:
  <SELECTOR>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive find senders`

```
Usage: stlive find senders [OPTIONS] <SELECTOR>

Arguments:
  <SELECTOR>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive class`

```
Classes

Usage: stlive class [OPTIONS] <COMMAND>

Commands:
  show    Hierarchy, variables, methods ('Foo' or 'Foo class')
  create  Create (or redefine) a class
  help    Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive class show`

```
Hierarchy, variables, methods ('Foo' or 'Foo class')

Usage: stlive class show [OPTIONS] <NAME>

Arguments:
  <NAME>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --offset <OFFSET>        
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive class create`

```
Create (or redefine) a class

Usage: stlive class create [OPTIONS] <NAME>

Arguments:
  <NAME>  

Options:
  -i, --instance <INSTANCE>      Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --superclass <SUPERCLASS>  
      --ivars <IVARS>            Space separated instance variable names
      --state-dir <STATE_DIR>    Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --package <PACKAGE>        
      --pretty                   Pretty-print JSON
      --text                     Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                     Print help
```

## `stlive method`

```
Methods

Usage: stlive method [OPTIONS] <COMMAND>

Commands:
  show     Source of Class>>selector
  compile  Compile a method into a class (source from --source, --file or stdin)
  remove   Remove a method (destructive)
  help     Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive method show`

```
Source of Class>>selector

Usage: stlive method show [OPTIONS] <TARGET>

Arguments:
  <TARGET>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive method compile`

```
Compile a method into a class (source from --source, --file or stdin)

Usage: stlive method compile [OPTIONS] <CLASS> [SOURCE_ARG]

Arguments:
  <CLASS>       
  [SOURCE_ARG]  Method source; '-' reads stdin (same as omitting it)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --source <SOURCE>        
      --file <FILE>            Read the source from a file ('-' = stdin)
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --protocol <PROTOCOL>    
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive method remove`

```
Remove a method (destructive)

Usage: stlive method remove [OPTIONS] <CLASS> <SELECTOR>

Arguments:
  <CLASS>     
  <SELECTOR>  

Options:
      --force                  
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive changes`

```
Recorded changes made through this CLI

Usage: stlive changes [OPTIONS] <COMMAND>

Commands:
  list  
  show  
  help  Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive changes list`

```
Usage: stlive changes list [OPTIONS]

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --offset <OFFSET>        
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive changes show`

```
Usage: stlive changes show [OPTIONS] <INDEX>

Arguments:
  <INDEX>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive test`

```
Tests

Usage: stlive test [OPTIONS] <COMMAND>

Commands:
  list  List test classes (substring or glob on class or package name)
  run   Run a TestCase class, 'Class>>testSelector', or a package (also repeats single tests)
  help  Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive test list`

```
List test classes (substring or glob on class or package name)

Usage: stlive test list [OPTIONS] [PATTERN]

Arguments:
  [PATTERN]  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive test run`

```
Run a TestCase class, 'Class>>testSelector', or a package (also repeats single tests)

Usage: stlive test run [OPTIONS] <TARGET>

Arguments:
  <TARGET>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --timeout <TIMEOUT>      Milliseconds per test
      --keep-sessions          Keep every failing test's debug session (default: one per failure group, the rest are ended)
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --discard-sessions       End all sessions of this run, including the representative one per group
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive package`

```
Packages

Usage: stlive package [OPTIONS] <COMMAND>

Commands:
  list  List packages (substring or glob like 'Mapless*'; default all)
  help  Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive package list`

```
List packages (substring or glob like 'Mapless*'; default all)

Usage: stlive package list [OPTIONS] [PATTERN]

Arguments:
  [PATTERN]  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive debug`

```
Debug sessions

Usage: stlive debug [OPTIONS] <COMMAND>

Commands:
  list       Sessions (ended ones only with --all)
  inspect    Compact description of a session
  frames     A page of frames (default 5)
  frame      One frame in detail (source, receiver, variables)
  locals     Arguments and temporaries of a frame
  receiver   Receiver of a frame as an inspectable object
  eval       Evaluate code in a frame (temporaries visible; --frame N, default 0 = top frame)
  resume     CONTINUE the suspended execution (does not re-run; does not prove a fix)
  restart    Unwind to a frame and RESTART it with the current method code
  return     Make a frame return a value and continue
  rerun      RE-RUN the original operation from scratch with current code
  terminate  Terminate a session (runs unwind blocks) or --all
  help       Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug list`

```
Sessions (ended ones only with --all)

Usage: stlive debug list [OPTIONS]

Options:
      --all                    
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug inspect`

```
Compact description of a session

Usage: stlive debug inspect [OPTIONS] <SESSION>

Arguments:
  <SESSION>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug frames`

```
A page of frames (default 5)

Usage: stlive debug frames [OPTIONS] <SESSION>

Arguments:
  <SESSION>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --offset <OFFSET>        
      --limit <LIMIT>          
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug frame`

```
One frame in detail (source, receiver, variables)

Usage: stlive debug frame [OPTIONS] <SESSION> [INDEX]

Arguments:
  <SESSION>  
  [INDEX]    

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --no-source              Omit the method source
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug locals`

```
Arguments and temporaries of a frame

Usage: stlive debug locals [OPTIONS] <SESSION>

Arguments:
  <SESSION>  

Options:
      --frame <FRAME>          [default: 0]
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug receiver`

```
Receiver of a frame as an inspectable object

Usage: stlive debug receiver [OPTIONS] <SESSION>

Arguments:
  <SESSION>  

Options:
      --frame <FRAME>          [default: 0]
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug eval`

```
Evaluate code in a frame (temporaries visible; --frame N, default 0 = top frame)

Usage: stlive debug eval [OPTIONS] <SESSION> <CODE>

Arguments:
  <SESSION>  
  <CODE>     

Options:
      --frame <FRAME>          [default: 0]
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --timeout <TIMEOUT>      
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug resume`

```
CONTINUE the suspended execution (does not re-run; does not prove a fix)

Usage: stlive debug resume [OPTIONS] <SESSION>

Arguments:
  <SESSION>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --value <VALUE>          Smalltalk expression whose value the failed send answers
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --timeout <TIMEOUT>      
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug restart`

```
Unwind to a frame and RESTART it with the current method code

Usage: stlive debug restart [OPTIONS] <SESSION>

Arguments:
  <SESSION>  

Options:
      --frame <FRAME>          [default: 0]
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --timeout <TIMEOUT>      
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug return`

```
Make a frame return a value and continue

Usage: stlive debug return [OPTIONS] <SESSION> <VALUE>

Arguments:
  <SESSION>  
  <VALUE>    

Options:
      --frame <FRAME>          [default: 0]
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --timeout <TIMEOUT>      
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug rerun`

```
RE-RUN the original operation from scratch with current code

Usage: stlive debug rerun [OPTIONS] <SESSION>

Arguments:
  <SESSION>  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive debug terminate`

```
Terminate a session (runs unwind blocks) or --all

Usage: stlive debug terminate [OPTIONS] [SESSION]

Arguments:
  [SESSION]  

Options:
      --all                    
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive image`

```
Image persistence, info and cloning

Usage: stlive image [OPTIONS] <COMMAND>

Commands:
  save   Snapshot the image to disk
  info   Version, image file, loaded non-system packages, unsaved changes, sessions
  clone  Save this instance's image and register a copy as a new instance (shares VM and sources file)
  help   Print this message or the help of the given subcommand(s)

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive image save`

```
Snapshot the image to disk

Usage: stlive image save [OPTIONS]

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive image info`

```
Version, image file, loaded non-system packages, unsaved changes, sessions

Usage: stlive image info [OPTIONS]

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

### `stlive image clone`

```
Save this instance's image and register a copy as a new instance (shares VM and sources file)

Usage: stlive image clone [OPTIONS] <NAME>

Arguments:
  <NAME>  Name of the new instance

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --start                  Start it right away
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive save`

```
Write changes made through this CLI back to Tonel files (minimal per-method diffs)

Usage: stlive save [OPTIONS] --dir <DIR>

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --package <PACKAGE>      Package whose changes are written
      --all                    Write the changes of every package (one folder each)
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --dir <DIR>              Source directory containing one folder per package (e.g. the repo's src/)
      --pretty                 Pretty-print JSON
      --dry-run                Show what would be written
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```

## `stlive load`

```
Load a Metacello baseline; reports the packages that appeared

Usage: stlive load [OPTIONS] --repository <REPOSITORY> <BASELINE>

Arguments:
  <BASELINE>  Baseline name without the BaselineOf prefix, e.g. Mapless

Options:
  -i, --instance <INSTANCE>      Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --repository <REPOSITORY>  Repository, e.g. tonel:///abs/path/src or github://user/repo:master/src
      --group <GROUPS>           Groups to load (repeatable); default group if none
      --state-dir <STATE_DIR>    Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                   Pretty-print JSON
      --timeout <TIMEOUT>        
      --text                     Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                     Print help
```

## `stlive raw`

```
Send a raw command: stlive raw debug.frames session=dbg-1 limit=3

Usage: stlive raw [OPTIONS] <CMD> [ARGS]...

Arguments:
  <CMD>      
  [ARGS]...  

Options:
  -i, --instance <INSTANCE>    Instance name (several images can run side by side) [env: STLIVE_INSTANCE=] [default: default]
      --state-dir <STATE_DIR>  Directory holding instance state (port file, pid, log, config) [env: STLIVE_DIR=]
      --pretty                 Pretty-print JSON
      --text                   Human-readable indented text instead of JSON (exit codes are unchanged)
  -h, --help                   Print help
```
