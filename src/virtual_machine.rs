//! Virtual Machine for running instructions
//!
//! A VirtualMachine manages threads, runs instructions, starts/terminates
//! threads and so on. VirtualMachine instances are fully self contained
//! allowing multiple instances to run fully isolated in the same process.

use std::collections::HashSet;
use std::io::{self, Write, Read, Seek, SeekFrom};
use std::fs::OpenOptions;
use std::thread;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::channel;

use bytecode_parser;
use call_frame::CallFrame;
use compiled_code::RcCompiledCode;
use errors;
use instruction::{InstructionType, Instruction};
use memory_manager::{MemoryManager, RcMemoryManager};
use object::RcObject;
use object_value;
use virtual_machine_methods::VirtualMachineMethods;
use virtual_machine_result::*;
use thread::{Thread, RcThread, JoinHandle as ThreadJoinHandle};
use thread_list::ThreadList;

/// A reference counted VirtualMachine.
pub type RcVirtualMachine = Arc<VirtualMachine>;

/// Structure representing a single VM instance.
pub struct VirtualMachine {
    /// All threads that are currently active.
    threads: RwLock<ThreadList>,

    /// The struct for allocating/managing memory.
    memory_manager: RcMemoryManager,

    /// The status of the VM when exiting.
    exit_status: RwLock<Result<(), ()>>,

    /// The files executed by the "run_file" instruction(s)
    executed_files: RwLock<HashSet<String>>
}

impl VirtualMachine {
    pub fn new() -> RcVirtualMachine {
        let vm = VirtualMachine {
            threads: RwLock::new(ThreadList::new()),
            memory_manager: MemoryManager::new(),
            exit_status: RwLock::new(Ok(())),
            executed_files: RwLock::new(HashSet::new())
        };

        Arc::new(vm)
    }

    fn integer_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).integer_prototype()
    }

    fn float_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).float_prototype()
    }

    fn string_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).string_prototype()
    }

    fn array_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).array_prototype()
    }

    fn thread_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).thread_prototype()
    }

    fn true_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).true_prototype()
    }

    fn false_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).false_prototype()
    }

    fn file_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).file_prototype()
    }

    fn method_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).method_prototype()
    }

    fn compiled_code_prototype(&self) -> RcObject {
        read_lock!(self.memory_manager).compiled_code_prototype()
    }

    fn false_object(&self) -> RcObject {
        read_lock!(self.memory_manager).false_object()
    }

    fn true_object(&self) -> RcObject {
        read_lock!(self.memory_manager).true_object()
    }

    fn allocate(&self, value: object_value::ObjectValue, prototype: RcObject) -> RcObject {
        write_lock!(self.memory_manager).allocate(value, prototype)
    }

    fn allocate_error(&self, name: &'static str) -> RcObject {
        write_lock!(self.memory_manager).allocate_error(name)
    }

    fn allocate_thread(&self, code: RcCompiledCode,
                       handle: Option<ThreadJoinHandle>,
                       main_thread: bool) -> RcObject {
        let vm_thread = Thread::from_code(code, handle);

        if main_thread {
            vm_thread.set_main();
        }

        let thread_obj = write_lock!(self.memory_manager)
            .allocate_thread(vm_thread);

        write_lock!(self.threads).add(thread_obj.clone());

        thread_obj
    }
}

impl VirtualMachineMethods for RcVirtualMachine {
    fn start(&self, code: RcCompiledCode) -> Result<(), ()> {
        let thread_obj = self.allocate_thread(code.clone(), None, true);

        self.run_thread(thread_obj, code.clone());

        *read_lock!(self.exit_status)
    }

    fn run(&self, thread: RcThread, code: RcCompiledCode) -> OptionObjectResult {
        if thread.should_stop() {
            return Ok(None);
        }

        let mut skip_until: Option<usize> = None;
        let mut retval = None;

        let mut index = 0;
        let count = code.instructions.len();

        while index < count {
            let ref instruction = code.instructions[index];

            if skip_until.is_some() {
                if index < skip_until.unwrap() {
                    continue;
                }
                else {
                    skip_until = None;
                }
            }

            // Incremented _before_ the instructions so that the "goto"
            // instruction can overwrite it.
            index += 1;

            match instruction.instruction_type {
                InstructionType::SetInteger => {
                    run!(self, ins_set_integer, thread, code, instruction);
                },
                InstructionType::SetFloat => {
                    run!(self, ins_set_float, thread, code, instruction);
                },
                InstructionType::SetString => {
                    run!(self, ins_set_string, thread, code, instruction);
                },
                InstructionType::SetObject => {
                    run!(self, ins_set_object, thread, code, instruction);
                },
                InstructionType::SetArray => {
                    run!(self, ins_set_array, thread, code, instruction);
                },
                InstructionType::SetName => {
                    run!(self, ins_set_name, thread, code, instruction);
                },
                InstructionType::GetIntegerPrototype => {
                    run!(self, ins_get_integer_prototype, thread, code,
                         instruction);
                },
                InstructionType::GetFloatPrototype => {
                    run!(self, ins_get_float_prototype, thread, code,
                         instruction);
                },
                InstructionType::GetStringPrototype => {
                    run!(self, ins_get_string_prototype, thread, code,
                         instruction);
                },
                InstructionType::GetArrayPrototype => {
                    run!(self, ins_get_array_prototype, thread, code,
                         instruction);
                },
                InstructionType::GetThreadPrototype => {
                    run!(self, ins_get_thread_prototype, thread, code,
                         instruction);
                },
                InstructionType::GetTruePrototype => {
                    run!(self, ins_get_true_prototype, thread, code,
                         instruction);
                },
                InstructionType::GetFalsePrototype => {
                    run!(self, ins_get_false_prototype, thread, code,
                         instruction);
                },
                InstructionType::GetMethodPrototype => {
                    run!(self, ins_get_method_prototype, thread, code,
                         instruction);
                },
                InstructionType::GetCompiledCodePrototype => {
                    run!(self, ins_get_compiled_code_prototype, thread, code,
                         instruction);
                },
                InstructionType::SetTrue => {
                    run!(self, ins_set_true, thread, code, instruction);
                },
                InstructionType::SetFalse => {
                    run!(self, ins_set_false, thread, code, instruction);
                },
                InstructionType::SetLocal => {
                    run!(self, ins_set_local, thread, code, instruction);
                },
                InstructionType::GetLocal => {
                    run!(self, ins_get_local, thread, code, instruction);
                },
                InstructionType::SetConst => {
                    run!(self, ins_set_const, thread, code, instruction);
                },
                InstructionType::GetConst => {
                    run!(self, ins_get_const, thread, code, instruction);
                },
                InstructionType::SetAttr => {
                    run!(self, ins_set_attr, thread, code, instruction);
                },
                InstructionType::GetAttr => {
                    run!(self, ins_get_attr, thread, code, instruction);
                },
                InstructionType::SetCompiledCode => {
                    run!(self, ins_set_compiled_code, thread, code,
                         instruction);
                },
                InstructionType::Send => {
                    run!(self, ins_send, thread, code, instruction);
                },
                InstructionType::Return => {
                    retval = run!(self, ins_return, thread, code, instruction);
                },
                InstructionType::GotoIfFalse => {
                    skip_until = run!(self, ins_goto_if_false, thread, code,
                                      instruction);
                },
                InstructionType::GotoIfTrue => {
                    skip_until = run!(self, ins_goto_if_true, thread, code,
                                      instruction);
                },
                InstructionType::Goto => {
                    index = run!(self, ins_goto, thread, code, instruction);
                },
                InstructionType::DefMethod => {
                    run!(self, ins_def_method, thread, code, instruction);
                },
                InstructionType::DefLiteralMethod => {
                    run!(self, ins_def_literal_method, thread, code,
                         instruction);
                },
                InstructionType::RunCode => {
                    run!(self, ins_run_code, thread, code, instruction);
                },
                InstructionType::GetToplevel => {
                    run!(self, ins_get_toplevel, thread, code, instruction);
                },
                InstructionType::IsError => {
                    run!(self, ins_is_error, thread, code, instruction);
                },
                InstructionType::ErrorToString => {
                    run!(self, ins_error_to_string, thread, code, instruction);
                },
                InstructionType::IntegerAdd => {
                    run!(self, ins_integer_add, thread, code, instruction);
                },
                InstructionType::IntegerDiv => {
                    run!(self, ins_integer_div, thread, code, instruction);
                },
                InstructionType::IntegerMul => {
                    run!(self, ins_integer_mul, thread, code, instruction);
                },
                InstructionType::IntegerSub => {
                    run!(self, ins_integer_sub, thread, code, instruction);
                },
                InstructionType::IntegerMod => {
                    run!(self, ins_integer_mod, thread, code, instruction);
                },
                InstructionType::IntegerToFloat => {
                    run!(self, ins_integer_to_float, thread, code, instruction);
                },
                InstructionType::IntegerToString => {
                    run!(self, ins_integer_to_string, thread, code,
                         instruction);
                },
                InstructionType::IntegerBitwiseAnd => {
                    run!(self, ins_integer_bitwise_and, thread, code,
                         instruction);
                },
                InstructionType::IntegerBitwiseOr => {
                    run!(self, ins_integer_bitwise_or, thread, code,
                         instruction);
                },
                InstructionType::IntegerBitwiseXor => {
                    run!(self, ins_integer_bitwise_xor, thread, code,
                         instruction);
                },
                InstructionType::IntegerShiftLeft => {
                    run!(self, ins_integer_shift_left, thread, code,
                         instruction);
                },
                InstructionType::IntegerShiftRight => {
                    run!(self, ins_integer_shift_right, thread, code,
                         instruction);
                },
                InstructionType::IntegerSmaller => {
                    run!(self, ins_integer_smaller, thread, code, instruction);
                },
                InstructionType::IntegerGreater => {
                    run!(self, ins_integer_greater, thread, code, instruction);
                },
                InstructionType::IntegerEquals => {
                    run!(self, ins_integer_equals, thread, code, instruction);
                },
                InstructionType::StartThread => {
                    run!(self, ins_start_thread, thread, code, instruction);
                },
                InstructionType::FloatAdd => {
                    run!(self, ins_float_add, thread, code, instruction);
                },
                InstructionType::FloatMul => {
                    run!(self, ins_float_mul, thread, code, instruction);
                },
                InstructionType::FloatDiv => {
                    run!(self, ins_float_div, thread, code, instruction);
                },
                InstructionType::FloatSub => {
                    run!(self, ins_float_sub, thread, code, instruction);
                },
                InstructionType::FloatMod => {
                    run!(self, ins_float_mod, thread, code, instruction);
                },
                InstructionType::FloatToInteger => {
                    run!(self, ins_float_to_integer, thread, code, instruction);
                },
                InstructionType::FloatToString => {
                    run!(self, ins_float_to_string, thread, code, instruction);
                },
                InstructionType::FloatSmaller => {
                    run!(self, ins_float_smaller, thread, code, instruction);
                },
                InstructionType::FloatGreater => {
                    run!(self, ins_float_greater, thread, code, instruction);
                },
                InstructionType::FloatEquals => {
                    run!(self, ins_float_equals, thread, code, instruction);
                },
                InstructionType::ArrayInsert => {
                    run!(self, ins_array_insert, thread, code, instruction);
                },
                InstructionType::ArrayAt => {
                    run!(self, ins_array_at, thread, code, instruction);
                },
                InstructionType::ArrayRemove => {
                    run!(self, ins_array_remove, thread, code, instruction);
                },
                InstructionType::ArrayLength => {
                    run!(self, ins_array_length, thread, code, instruction);
                },
                InstructionType::ArrayClear => {
                    run!(self, ins_array_clear, thread, code, instruction);
                },
                InstructionType::StringToLower => {
                    run!(self, ins_string_to_lower, thread, code, instruction);
                },
                InstructionType::StringToUpper => {
                    run!(self, ins_string_to_upper, thread, code, instruction);
                },
                InstructionType::StringEquals => {
                    run!(self, ins_string_equals, thread, code, instruction);
                },
                InstructionType::StringToBytes => {
                    run!(self, ins_string_to_bytes, thread, code, instruction);
                },
                InstructionType::StringFromBytes => {
                    run!(self, ins_string_from_bytes, thread, code, instruction);
                },
                InstructionType::StringLength => {
                    run!(self, ins_string_length, thread, code, instruction);
                },
                InstructionType::StringSize => {
                    run!(self, ins_string_size, thread, code, instruction);
                },
                InstructionType::StdoutWrite => {
                    run!(self, ins_stdout_write, thread, code, instruction);
                },
                InstructionType::StderrWrite => {
                    run!(self, ins_stderr_write, thread, code, instruction);
                },
                InstructionType::StdinRead => {
                    run!(self, ins_stdin_read, thread, code, instruction);
                },
                InstructionType::StdinReadLine => {
                    run!(self, ins_stdin_read_line, thread, code, instruction);
                },
                InstructionType::FileOpen => {
                    run!(self, ins_file_open, thread, code, instruction);
                },
                InstructionType::FileWrite => {
                    run!(self, ins_file_write, thread, code, instruction);
                },
                InstructionType::FileRead => {
                    run!(self, ins_file_read, thread, code, instruction);
                },
                InstructionType::FileReadLine => {
                    run!(self, ins_file_read_line, thread, code, instruction);
                },
                InstructionType::FileFlush => {
                    run!(self, ins_file_flush, thread, code, instruction);
                },
                InstructionType::FileSize => {
                    run!(self, ins_file_size, thread, code, instruction);
                },
                InstructionType::FileSeek => {
                    run!(self, ins_file_seek, thread, code, instruction);
                },
                InstructionType::RunFileFast => {
                    run!(self, ins_run_file_fast, thread, code, instruction);
                }
            };
        }

        Ok(retval)
    }

    fn ins_set_integer(&self, thread: RcThread, code: RcCompiledCode,
                       instruction: &Instruction) -> EmptyResult {
        let slot  = try!(instruction.arg(0));
        let index = try!(instruction.arg(1));
        let value = *try!(code.integer(index));

        let obj = self.allocate(object_value::integer(value),
                                self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_set_float(&self, thread: RcThread, code: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot  = try!(instruction.arg(0));
        let index = try!(instruction.arg(1));
        let value = *try!(code.float(index));

        let obj = self.allocate(object_value::float(value),
                                self.float_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_set_string(&self, thread: RcThread, code: RcCompiledCode,
                      instruction: &Instruction) -> EmptyResult {
        let slot  = try!(instruction.arg(0));
        let index = try!(instruction.arg(1));
        let value = try!(code.string(index));

        let obj = self.allocate(object_value::string(value.clone()),
                                self.string_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_set_object(&self, thread: RcThread, _: RcCompiledCode,
                      instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        let proto_index_opt = instruction.arguments.get(1);

        let obj = write_lock!(self.memory_manager)
            .new_object(object_value::none());

        if proto_index_opt.is_some() {
            let proto_index = *proto_index_opt.unwrap();
            let proto       = try!(thread.get_register(proto_index));

            write_lock!(obj).set_prototype(proto);
        }

        write_lock!(self.memory_manager)
            .allocate_prepared(obj.clone());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_set_array(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot      = try!(instruction.arg(0));
        let val_count = try!(instruction.arg(1));

        let values = try!(
            self.collect_arguments(thread.clone(), instruction, 2, val_count)
        );

        let obj = self.allocate(object_value::array(values),
                                self.array_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_set_name(&self, thread: RcThread, code: RcCompiledCode,
                    instruction: &Instruction) -> EmptyResult {
        let name_index = try!(instruction.arg(1));

        let obj  = instruction_object!(instruction, thread, 0);
        let name = try!(code.string(name_index));

        write_lock!(obj).set_name(name.clone());

        Ok(())
    }

    fn ins_get_integer_prototype(&self, thread: RcThread, _: RcCompiledCode,
                                 instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.integer_prototype());

        Ok(())
    }

    fn ins_get_float_prototype(&self, thread: RcThread, _: RcCompiledCode,
                               instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.float_prototype());

        Ok(())
    }

    fn ins_get_string_prototype(&self, thread: RcThread, _: RcCompiledCode,
                                instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.string_prototype());

        Ok(())
    }

    fn ins_get_array_prototype(&self, thread: RcThread, _: RcCompiledCode,
                               instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.array_prototype());

        Ok(())
    }

    fn ins_get_thread_prototype(&self, thread: RcThread, _: RcCompiledCode,
                                instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.thread_prototype());

        Ok(())
    }

    fn ins_get_true_prototype(&self, thread: RcThread, _: RcCompiledCode,
                              instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.true_prototype());

        Ok(())
    }

    fn ins_get_false_prototype(&self, thread: RcThread, _: RcCompiledCode,
                              instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.false_prototype());

        Ok(())
    }

    fn ins_get_method_prototype(&self, thread: RcThread, _: RcCompiledCode,
                                instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.method_prototype());

        Ok(())
    }

    fn ins_get_compiled_code_prototype(&self, thread: RcThread, _: RcCompiledCode,
                                       instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.compiled_code_prototype());

        Ok(())
    }

    fn ins_set_true(&self, thread: RcThread, _: RcCompiledCode,
                    instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.true_object());

        Ok(())
    }

    fn ins_set_false(&self, thread: RcThread, _: RcCompiledCode,
                    instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        thread.set_register(slot, self.false_object());

        Ok(())
    }

    fn ins_set_local(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let local_index = try!(instruction.arg(0));
        let object      = instruction_object!(instruction, thread, 1);

        thread.set_local(local_index, object);

        Ok(())
    }

    fn ins_get_local(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot_index = try!(instruction.arg(0));
        let object     = instruction_object!(instruction, thread, 1);

        thread.set_register(slot_index, object);

        Ok(())
    }

    fn ins_set_const(&self, thread: RcThread, code: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let name_index = try!(instruction.arg(2));
        let target     = instruction_object!(instruction, thread, 0);
        let source     = instruction_object!(instruction, thread, 1);
        let name       = try!(code.string(name_index));

        write_lock!(target).add_constant(name.clone(), source);

        Ok(())
    }

    fn ins_get_const(&self, thread: RcThread, code: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let index      = try!(instruction.arg(0));
        let src        = instruction_object!(instruction, thread, 1);
        let name_index = try!(instruction.arg(2));
        let name       = try!(code.string(name_index));

        let object = try!(
            read_lock!(src).lookup_constant(name)
                .ok_or(format!("Undefined constant {}", name))
        );

        thread.set_register(index, object);

        Ok(())
    }

    fn ins_set_attr(&self, thread: RcThread, _: RcCompiledCode,
                    instruction: &Instruction) -> EmptyResult {
        let target_object = instruction_object!(instruction, thread, 0);
        let source_object = instruction_object!(instruction, thread, 1);
        let name_lock     = instruction_object!(instruction, thread, 2);

        let name_obj = read_lock!(name_lock);

        ensure_strings!(name_obj);

        let name = name_obj.value.as_string();

        write_lock!(target_object)
            .add_attribute(name.clone(), source_object);

        Ok(())
    }

    fn ins_get_attr(&self, thread: RcThread, _: RcCompiledCode,
                    instruction: &Instruction) -> EmptyResult {
        let target_index = try!(instruction.arg(0));
        let source       = instruction_object!(instruction, thread, 1);
        let name_lock    = instruction_object!(instruction, thread, 2);

        let name_obj = read_lock!(name_lock);

        ensure_strings!(name_obj);

        let name = name_obj.value.as_string();

        let attr = try!(
            read_lock!(source).lookup_attribute(name)
                .ok_or(format!("undefined attribute {}", name))
        );

        thread.set_register(target_index, attr);

        Ok(())
    }

    fn ins_set_compiled_code(&self, thread: RcThread, code: RcCompiledCode,
                             instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let cc_index = try!(instruction.arg(1));

        let cc = try!(code.code_object(cc_index));

        let obj = self.allocate(object_value::compiled_code(cc.clone()),
                                self.compiled_code_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_send(&self, thread: RcThread, code: RcCompiledCode,
                instruction: &Instruction) -> EmptyResult {
        let result_slot   = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let name_index    = try!(instruction.arg(2));
        let allow_private = try!(instruction.arg(3));
        let arg_count     = try!(instruction.arg(4));
        let name          = try!(code.string(name_index));

        let receiver = read_lock!(receiver_lock);

        let method_lock = try!(
            receiver.lookup_method(name)
                .ok_or(receiver.undefined_method_error(name))
        );

        let method_obj = read_lock!(method_lock);

        ensure_compiled_code!(method_obj);

        let method_code = method_obj.value.as_compiled_code();

        if method_code.is_private() && allow_private == 0 {
            return Err(receiver.private_method_error(name));
        }

        let mut arguments = try!(
            self.collect_arguments(thread.clone(), instruction, 5, arg_count)
        );

        if arguments.len() != method_code.required_arguments {
            return Err(format!(
                "{} requires {} arguments, {} given",
                name,
                method_code.required_arguments,
                arguments.len()
            ));
        }

        // Expose the receiver as "self" to the method
        arguments.insert(0, receiver_lock.clone());

        let retval = try!(
            self.run_code(thread.clone(), method_code, arguments)
        );

        if retval.is_some() {
            thread.set_register(result_slot, retval.unwrap());
        }

        Ok(())
    }

    fn ins_return(&self, thread: RcThread, _: RcCompiledCode,
                  instruction: &Instruction) -> OptionObjectResult {
        let slot = try!(instruction.arg(0));

        Ok(thread.get_register_option(slot))
    }

    fn ins_goto_if_false(&self, thread: RcThread, _: RcCompiledCode,
                         instruction: &Instruction) -> OptionIntegerResult {
        let go_to      = try!(instruction.arg(0));
        let value_slot = try!(instruction.arg(1));
        let value      = thread.get_register_option(value_slot);

        let matched = match value {
            Some(obj) => {
                if read_lock!(obj).truthy() {
                    None
                }
                else {
                    Some(go_to)
                }
            },
            None => { Some(go_to) }
        };

        Ok(matched)
    }

    fn ins_goto_if_true(&self, thread: RcThread, _: RcCompiledCode,
                       instruction: &Instruction) -> OptionIntegerResult {
        let go_to      = try!(instruction.arg(0));
        let value_slot = try!(instruction.arg(1));
        let value      = thread.get_register_option(value_slot);

        let matched = match value {
            Some(obj) => {
                if read_lock!(obj).truthy() {
                    Some(go_to)
                }
                else {
                    None
                }
            },
            None => { None }
        };

        Ok(matched)
    }

    fn ins_goto(&self, _: RcThread, _: RcCompiledCode,
                instruction: &Instruction) -> IntegerResult {
        let go_to = try!(instruction.arg(0));

        Ok(go_to)
    }

    fn ins_def_method(&self, thread: RcThread, _: RcCompiledCode,
                      instruction: &Instruction) -> EmptyResult {
        let receiver_lock = instruction_object!(instruction, thread, 0);
        let name_lock     = instruction_object!(instruction, thread, 1);
        let cc_lock       = instruction_object!(instruction, thread, 2);

        let mut receiver = write_lock!(receiver_lock);
        let name_obj     = read_lock!(name_lock);
        let cc_obj       = read_lock!(cc_lock);

        ensure_strings!(name_obj);
        ensure_compiled_code!(cc_obj);

        let name = name_obj.value.as_string();
        let cc   = cc_obj.value.as_compiled_code();

        let method = self.allocate(object_value::compiled_code(cc),
                                   self.method_prototype());

        receiver.add_method(name.clone(), method);

        Ok(())
    }

    fn ins_def_literal_method(&self, thread: RcThread, code: RcCompiledCode,
                              instruction: &Instruction) -> EmptyResult {
        let receiver_lock = instruction_object!(instruction, thread, 0);
        let name_index    = try!(instruction.arg(1));
        let cc_index      = try!(instruction.arg(2));

        let name = try!(code.string(name_index));
        let cc   = try!(code.code_object(cc_index));

        let mut receiver = write_lock!(receiver_lock);

        let method = self.allocate(object_value::compiled_code(cc.clone()),
                                   self.method_prototype());

        receiver.add_method(name.clone(), method);

        Ok(())
    }

    fn ins_run_code(&self, thread: RcThread, _: RcCompiledCode,
                    instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let cc_lock  = instruction_object!(instruction, thread, 1);
        let arg_lock = instruction_object!(instruction, thread, 2);

        let cc_obj  = read_lock!(cc_lock);
        let arg_obj = read_lock!(arg_lock);

        ensure_compiled_code!(cc_obj);
        ensure_integers!(arg_obj);

        let arg_count = arg_obj.value.as_integer() as usize;
        let code_obj  = cc_obj.value.as_compiled_code();

        let arguments = try!(
            self.collect_arguments(thread.clone(), instruction, 3, arg_count)
        );

        let retval = try!(self.run_code(thread.clone(), code_obj, arguments));

        if retval.is_some() {
            thread.set_register(slot, retval.unwrap());
        }

        Ok(())
    }

    fn ins_get_toplevel(&self, thread: RcThread, _: RcCompiledCode,
                        instruction: &Instruction) -> EmptyResult {
        let slot = try!(instruction.arg(0));

        let top_level = read_lock!(self.memory_manager).top_level.clone();

        thread.set_register(slot, top_level);

        Ok(())
    }

    fn ins_is_error(&self, thread: RcThread, _: RcCompiledCode,
                    instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let obj_lock = instruction_object!(instruction, thread, 1);
        let obj      = read_lock!(obj_lock);

        let result = if obj.value.is_error() {
            self.true_object()
        }
        else {
            self.false_object()
        };

        thread.set_register(slot, result);

        Ok(())
    }

    fn ins_error_to_string(&self, thread: RcThread, _: RcCompiledCode,
                           instruction: &Instruction) -> EmptyResult {
        let slot       = try!(instruction.arg(0));
        let error_lock = instruction_object!(instruction, thread, 1);
        let error      = read_lock!(error_lock);

        let proto  = self.string_prototype();
        let string = error.value.as_error().clone();
        let result = self.allocate(object_value::string(string), proto);

        thread.set_register(slot, result);

        Ok(())
    }

    fn ins_integer_add(&self, thread: RcThread, _: RcCompiledCode,
                       instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() + arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_div(&self, thread: RcThread, _: RcCompiledCode,
                       instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() / arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_mul(&self, thread: RcThread, _: RcCompiledCode,
                       instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() * arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_sub(&self, thread: RcThread, _: RcCompiledCode,
                       instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() - arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_mod(&self, thread: RcThread, _: RcCompiledCode,
                       instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() % arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_to_float(&self, thread: RcThread, _: RcCompiledCode,
                            instruction: &Instruction) -> EmptyResult {
        let slot         = try!(instruction.arg(0));
        let integer_lock = instruction_object!(instruction, thread, 1);
        let integer      = read_lock!(integer_lock);

        ensure_integers!(integer);

        let result = integer.value.as_integer() as f64;
        let obj    = self.allocate(object_value::float(result),
                                   self.float_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_to_string(&self, thread: RcThread, _: RcCompiledCode,
                             instruction: &Instruction) -> EmptyResult {
        let slot         = try!(instruction.arg(0));
        let integer_lock = instruction_object!(instruction, thread, 1);

        let integer = read_lock!(integer_lock);

        ensure_integers!(integer);

        let result = integer.value.as_integer().to_string();
        let obj    = self.allocate(object_value::string(result),
                                   self.string_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_bitwise_and(&self, thread: RcThread, _: RcCompiledCode,
                               instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() & arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_bitwise_or(&self, thread: RcThread, _: RcCompiledCode,
                               instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() | arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_bitwise_xor(&self, thread: RcThread, _: RcCompiledCode,
                               instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() ^ arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_shift_left(&self, thread: RcThread, _: RcCompiledCode,
                               instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() << arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_shift_right(&self, thread: RcThread, _: RcCompiledCode,
                               instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() >> arg.value.as_integer();
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_integer_smaller(&self, thread: RcThread, _: RcCompiledCode,
                           instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() < arg.value.as_integer();

        let boolean = if result {
            self.true_object()
        }
        else {
            self.false_object()
        };

        thread.set_register(slot, boolean);

        Ok(())
    }

    fn ins_integer_greater(&self, thread: RcThread, _: RcCompiledCode,
                           instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() > arg.value.as_integer();

        let boolean = if result {
            self.true_object()
        }
        else {
            self.false_object()
        };

        thread.set_register(slot, boolean);

        Ok(())
    }

    fn ins_integer_equals(&self, thread: RcThread, _: RcCompiledCode,
                          instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_integers!(receiver, arg);

        let result = receiver.value.as_integer() == arg.value.as_integer();

        let boolean = if result {
            self.true_object()
        }
        else {
            self.false_object()
        };

        thread.set_register(slot, boolean);

        Ok(())
    }

    fn ins_start_thread(&self, thread: RcThread, code: RcCompiledCode,
                        instruction: &Instruction) -> EmptyResult {
        let slot        = try!(instruction.arg(0));
        let code_index  = try!(instruction.arg(1));
        let thread_code = try!(code.code_object(code_index)).clone();

        let thread_object = self.start_thread(thread_code);

        thread.set_register(slot, thread_object);

        Ok(())
    }

    fn ins_float_add(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_floats!(receiver, arg);

        let added = receiver.value.as_float() + arg.value.as_float();
        let obj   = self.allocate(object_value::float(added),
                                  self.float_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_float_mul(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_floats!(receiver, arg);

        let result = receiver.value.as_float() * arg.value.as_float();
        let obj    = self.allocate(object_value::float(result),
                                   self.float_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_float_div(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_floats!(receiver, arg);

        let result = receiver.value.as_float() / arg.value.as_float();
        let obj    = self.allocate(object_value::float(result),
                                   self.float_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_float_sub(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_floats!(receiver, arg);

        let result = receiver.value.as_float() - arg.value.as_float();
        let obj    = self.allocate(object_value::float(result),
                                   self.float_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_float_mod(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_floats!(receiver, arg);

        let result = receiver.value.as_float() % arg.value.as_float();
        let obj    = self.allocate(object_value::float(result),
                                   self.float_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_float_to_integer(&self, thread: RcThread, _: RcCompiledCode,
                            instruction: &Instruction) -> EmptyResult {
        let slot       = try!(instruction.arg(0));
        let float_lock = instruction_object!(instruction, thread, 1);
        let float      = read_lock!(float_lock);

        ensure_floats!(float);

        let result = float.value.as_float() as isize;
        let obj    = self.allocate(object_value::integer(result),
                                   self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_float_to_string(&self, thread: RcThread, _: RcCompiledCode,
                           instruction: &Instruction) -> EmptyResult {
        let slot       = try!(instruction.arg(0));
        let float_lock = instruction_object!(instruction, thread, 1);
        let float      = read_lock!(float_lock);

        ensure_floats!(float);

        let result = float.value.as_float().to_string();
        let obj    = self.allocate(object_value::string(result),
                                   self.string_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_float_smaller(&self, thread: RcThread, _: RcCompiledCode,
                         instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_floats!(receiver, arg);

        let result = receiver.value.as_float() < arg.value.as_float();

        let boolean = if result {
            self.true_object()
        }
        else {
            self.false_object()
        };

        thread.set_register(slot, boolean);

        Ok(())
    }

    fn ins_float_greater(&self, thread: RcThread, _: RcCompiledCode,
                         instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_floats!(receiver, arg);

        let result = receiver.value.as_float() > arg.value.as_float();

        let boolean = if result {
            self.true_object()
        }
        else {
            self.false_object()
        };

        thread.set_register(slot, boolean);

        Ok(())
    }

    fn ins_float_equals(&self, thread: RcThread, _: RcCompiledCode,
                        instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_floats!(receiver, arg);

        let result = receiver.value.as_float() == arg.value.as_float();

        let boolean = if result {
            self.true_object()
        }
        else {
            self.false_object()
        };

        thread.set_register(slot, boolean);

        Ok(())
    }

    fn ins_array_insert(&self, thread: RcThread, _: RcCompiledCode,
                        instruction: &Instruction) -> EmptyResult {
        let array_lock = instruction_object!(instruction, thread, 0);
        let index      = try!(instruction.arg(1));
        let value_lock = instruction_object!(instruction, thread, 2);
        let mut array  = write_lock!(array_lock);

        ensure_arrays!(array);

        let mut vector = array.value.as_array_mut();

        ensure_array_within_bounds!(vector, index);

        vector.insert(index, value_lock);

        Ok(())
    }

    fn ins_array_at(&self, thread: RcThread, _: RcCompiledCode,
                    instruction: &Instruction) -> EmptyResult {
        let slot       = try!(instruction.arg(0));
        let array_lock = instruction_object!(instruction, thread, 1);
        let index      = try!(instruction.arg(2));
        let array      = read_lock!(array_lock);

        ensure_arrays!(array);

        let vector = array.value.as_array();

        ensure_array_within_bounds!(vector, index);

        let value = vector[index].clone();

        thread.set_register(slot, value);

        Ok(())
    }

    fn ins_array_remove(&self, thread: RcThread, _: RcCompiledCode,
                        instruction: &Instruction) -> EmptyResult {
        let slot       = try!(instruction.arg(0));
        let array_lock = instruction_object!(instruction, thread, 1);
        let index      = try!(instruction.arg(1));
        let mut array  = write_lock!(array_lock);

        ensure_arrays!(array);

        let mut vector = array.value.as_array_mut();

        ensure_array_within_bounds!(vector, index);

        let value = vector.remove(index);

        thread.set_register(slot, value);

        Ok(())
    }

    fn ins_array_length(&self, thread: RcThread, _: RcCompiledCode,
                        instruction: &Instruction) -> EmptyResult {
        let slot       = try!(instruction.arg(0));
        let array_lock = instruction_object!(instruction, thread, 1);
        let array      = read_lock!(array_lock);

        ensure_arrays!(array);

        let vector = array.value.as_array();
        let length = vector.len() as isize;

        let obj = self.allocate(object_value::integer(length),
                                self.integer_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_array_clear(&self, thread: RcThread, _: RcCompiledCode,
                       instruction: &Instruction) -> EmptyResult {
        let array_lock = instruction_object!(instruction, thread, 0);
        let mut array  = write_lock!(array_lock);

        ensure_arrays!(array);

        let mut vector = array.value.as_array_mut();

        vector.clear();

        Ok(())
    }

    fn ins_string_to_lower(&self, thread: RcThread, _: RcCompiledCode,
                           instruction: &Instruction) -> EmptyResult {
        let slot        = try!(instruction.arg(0));
        let source_lock = instruction_object!(instruction, thread, 1);
        let source      = read_lock!(source_lock);

        ensure_strings!(source);

        let lower = source.value.as_string().to_lowercase();
        let obj   = self.allocate(object_value::string(lower),
                                  self.string_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_string_to_upper(&self, thread: RcThread, _: RcCompiledCode,
                           instruction: &Instruction) -> EmptyResult {
        let slot        = try!(instruction.arg(0));
        let source_lock = instruction_object!(instruction, thread, 1);
        let source      = read_lock!(source_lock);

        ensure_strings!(source);

        let upper = source.value.as_string().to_uppercase();
        let obj   = self.allocate(object_value::string(upper),
                                  self.string_prototype());

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_string_equals(&self, thread: RcThread, _: RcCompiledCode,
                         instruction: &Instruction) -> EmptyResult {
        let slot          = try!(instruction.arg(0));
        let receiver_lock = instruction_object!(instruction, thread, 1);
        let arg_lock      = instruction_object!(instruction, thread, 2);

        let receiver = read_lock!(receiver_lock);
        let arg      = read_lock!(arg_lock);

        ensure_strings!(receiver, arg);

        let result = receiver.value.as_string() == arg.value.as_string();

        let boolean = if result {
            self.true_object()
        }
        else {
            self.false_object()
        };

        thread.set_register(slot, boolean);

        Ok(())
    }

    fn ins_string_to_bytes(&self, thread: RcThread, _: RcCompiledCode,
                           instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let arg_lock = instruction_object!(instruction, thread, 1);
        let arg      = read_lock!(arg_lock);

        ensure_strings!(arg);

        let int_proto   = self.integer_prototype();
        let array_proto = self.array_prototype();

        let array = arg.value.as_string().as_bytes().iter().map(|&b| {
            self.allocate(object_value::integer(b as isize), int_proto.clone())
        }).collect::<Vec<_>>();

        let obj = self.allocate(object_value::array(array), array_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_string_from_bytes(&self, thread: RcThread, _: RcCompiledCode,
                             instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let arg_lock = instruction_object!(instruction, thread, 1);
        let arg      = read_lock!(arg_lock);

        ensure_arrays!(arg);

        let string_proto = self.string_prototype();
        let array        = arg.value.as_array();

        for int_lock in array.iter() {
            let int = read_lock!(int_lock);

            ensure_integers!(int);
        }

        let bytes = arg.value.as_array().iter().map(|ref int_lock| {
            read_lock!(int_lock).value.as_integer() as u8
        }).collect::<Vec<_>>();

        let string = try_error!(try_from_utf8!(bytes), self, thread, slot);
        let obj    = self.allocate(object_value::string(string), string_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_string_length(&self, thread: RcThread, _: RcCompiledCode,
                         instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let arg_lock = instruction_object!(instruction, thread, 1);
        let arg      = read_lock!(arg_lock);

        ensure_strings!(arg);

        let int_proto = self.integer_prototype();

        let length = arg.value.as_string().chars().count() as isize;
        let obj    = self.allocate(object_value::integer(length), int_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_string_size(&self, thread: RcThread, _: RcCompiledCode,
                       instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let arg_lock = instruction_object!(instruction, thread, 1);
        let arg      = read_lock!(arg_lock);

        ensure_strings!(arg);

        let int_proto = self.integer_prototype();

        let size = arg.value.as_string().len() as isize;
        let obj  = self.allocate(object_value::integer(size), int_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_stdout_write(&self, thread: RcThread, _: RcCompiledCode,
                        instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let arg_lock = instruction_object!(instruction, thread, 1);
        let arg      = read_lock!(arg_lock);

        ensure_strings!(arg);

        let int_proto  = self.integer_prototype();
        let mut stdout = io::stdout();

        let result = try_io!(stdout.write(arg.value.as_string().as_bytes()),
                             self, thread, slot);

        let obj = self.allocate(object_value::integer(result as isize),
                                int_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_stderr_write(&self, thread: RcThread, _: RcCompiledCode,
                        instruction: &Instruction) -> EmptyResult {
        let slot     = try!(instruction.arg(0));
        let arg_lock = instruction_object!(instruction, thread, 1);
        let arg      = read_lock!(arg_lock);

        ensure_strings!(arg);

        let int_proto  = self.integer_prototype();
        let mut stderr = io::stderr();

        let result = try_io!(stderr.write(arg.value.as_string().as_bytes()),
                             self, thread, slot);

        let obj = self.allocate(object_value::integer(result as isize),
                                int_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_stdin_read(&self, thread: RcThread, _: RcCompiledCode,
                      instruction: &Instruction) -> EmptyResult {
        let slot  = try!(instruction.arg(0));
        let proto = self.string_prototype();

        let mut buffer = file_reading_buffer!(instruction, thread, 1);

        try_io!(io::stdin().read_to_string(&mut buffer), self, thread, slot);

        let obj = self.allocate(object_value::string(buffer), proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_stdin_read_line(&self, thread: RcThread, _: RcCompiledCode,
                           instruction: &Instruction) -> EmptyResult {
        let slot  = try!(instruction.arg(0));
        let proto = self.string_prototype();

        let mut buffer = String::new();

        try_io!(io::stdin().read_line(&mut buffer), self, thread, slot);

        let obj = self.allocate(object_value::string(buffer), proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_file_open(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot      = try!(instruction.arg(0));
        let path_lock = instruction_object!(instruction, thread, 1);
        let mode_lock = instruction_object!(instruction, thread, 2);

        let file_proto = self.file_prototype();

        let path = read_lock!(path_lock);
        let mode = read_lock!(mode_lock);

        let path_string   = path.value.as_string();
        let mode_string   = mode.value.as_string().as_ref();
        let mut open_opts = OpenOptions::new();

        match mode_string {
            "r"  => open_opts.read(true),
            "r+" => open_opts.read(true).write(true).truncate(true).create(true),
            "w"  => open_opts.write(true).truncate(true).create(true),
            "w+" => open_opts.read(true).write(true).truncate(true).create(true),
            "a"  => open_opts.append(true).create(true),
            "a+" => open_opts.read(true).append(true).create(true),
            _    => set_error!(errors::IO_INVALID_OPEN_MODE, self, thread, slot)
        };

        let file = try_io!(open_opts.open(path_string), self, thread, slot);
        let obj  = self.allocate(object_value::file(file), file_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_file_write(&self, thread: RcThread, _: RcCompiledCode,
                      instruction: &Instruction) -> EmptyResult {
        let slot        = try!(instruction.arg(0));
        let file_lock   = instruction_object!(instruction, thread, 1);
        let string_lock = instruction_object!(instruction, thread, 2);

        let mut file = write_lock!(file_lock);
        let string   = read_lock!(string_lock);

        ensure_files!(file);
        ensure_strings!(string);

        let int_proto = self.integer_prototype();
        let mut file  = file.value.as_file_mut();
        let bytes     = string.value.as_string().as_bytes();

        let result = try_io!(file.write(bytes), self, thread, slot);

        let obj = self.allocate(object_value::integer(result as isize),
                                int_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_file_read(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot         = try!(instruction.arg(0));
        let file_lock    = instruction_object!(instruction, thread, 1);
        let mut file_obj = write_lock!(file_lock);

        ensure_files!(file_obj);

        let mut buffer = file_reading_buffer!(instruction, thread, 2);
        let int_proto  = self.integer_prototype();
        let mut file   = file_obj.value.as_file_mut();

        try_io!(file.read_to_string(&mut buffer), self, thread, slot);

        let obj = self.allocate(object_value::string(buffer), int_proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_file_read_line(&self, thread: RcThread, _: RcCompiledCode,
                          instruction: &Instruction) -> EmptyResult {
        let slot         = try!(instruction.arg(0));
        let file_lock    = instruction_object!(instruction, thread, 1);
        let mut file_obj = write_lock!(file_lock);

        ensure_files!(file_obj);

        let proto     = self.string_prototype();
        let mut file  = file_obj.value.as_file_mut();
        let mut bytes = Vec::new();

        for result in file.bytes() {
            let byte = try_io!(result, self, thread, slot);

            bytes.push(byte);

            if byte == 0xA {
                break;
            }
        }

        let string = try_error!(try_from_utf8!(bytes), self, thread, slot);
        let obj    = self.allocate(object_value::string(string), proto);

        thread.set_register(slot, obj);

        Ok(())
    }

    fn ins_file_flush(&self, thread: RcThread, _: RcCompiledCode,
                      instruction: &Instruction) -> EmptyResult {
        let slot         = try!(instruction.arg(0));
        let file_lock    = instruction_object!(instruction, thread, 1);
        let mut file_obj = write_lock!(file_lock);

        ensure_files!(file_obj);

        let mut file = file_obj.value.as_file_mut();

        try_io!(file.flush(), self, thread, slot);

        thread.set_register(slot, self.true_object());

        Ok(())
    }

    fn ins_file_size(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot      = try!(instruction.arg(0));
        let file_lock = instruction_object!(instruction, thread, 1);
        let file_obj  = read_lock!(file_lock);

        ensure_files!(file_obj);

        let file = file_obj.value.as_file();
        let meta = try_io!(file.metadata(), self, thread, slot);

        let size   = meta.len() as isize;
        let proto  = self.integer_prototype();
        let result = self.allocate(object_value::integer(size), proto);

        thread.set_register(slot, result);

        Ok(())
    }

    fn ins_file_seek(&self, thread: RcThread, _: RcCompiledCode,
                     instruction: &Instruction) -> EmptyResult {
        let slot        = try!(instruction.arg(0));
        let file_lock   = instruction_object!(instruction, thread, 1);
        let offset_lock = instruction_object!(instruction, thread, 2);

        let mut file_obj = write_lock!(file_lock);
        let offset_obj   = read_lock!(offset_lock);

        ensure_files!(file_obj);
        ensure_integers!(offset_obj);

        let mut file = file_obj.value.as_file_mut();
        let offset   = offset_obj.value.as_integer();

        ensure_positive_read_size!(offset);

        let seek_from  = SeekFrom::Start(offset as u64);
        let new_offset = try_io!(file.seek(seek_from), self, thread, slot);

        let proto  = self.integer_prototype();
        let result = self.allocate(object_value::integer(new_offset as isize),
                                   proto);

        thread.set_register(slot, result);

        Ok(())
    }

    fn ins_run_file_fast(&self, thread: RcThread, code: RcCompiledCode,
                         instruction: &Instruction) -> EmptyResult {
        let slot  = try!(instruction.arg(0));
        let index = try!(instruction.arg(1));
        let path  = try!(code.string(index));

        {
            let mut executed = self.executed_files.write().unwrap();

            if executed.contains(path) {
                return Ok(());
            }
            else {
                executed.insert(path.clone());
            }
        }

        match bytecode_parser::parse_file(path) {
            Ok(body) => {
                let res = try!(self.run_code(thread.clone(), body, Vec::new()));

                if res.is_some() {
                    thread.set_register(slot, res.unwrap());
                }

                Ok(())
            },
            Err(err) => Err(format!("Failed to parse {}: {:?}", path, err))
        }
    }

    fn error(&self, thread: RcThread, message: String) {
        let mut stderr = io::stderr();
        let mut error  = message.to_string();
        let frame      = read_lock!(thread.call_frame);

        *write_lock!(self.exit_status) = Err(());

        frame.each_frame(|frame| {
            error.push_str(&format!(
                "\n{} line {} in \"{}\"",
                frame.file,
                frame.line,
                frame.name
            ));
        });

        write!(&mut stderr, "Fatal error:\n\n{}\n\n", error).unwrap();

        stderr.flush().unwrap();
    }

    fn run_code(&self, thread: RcThread, code: RcCompiledCode,
                args: Vec<RcObject>) -> OptionObjectResult {
        // Scoped so the the RwLock is local to the block, allowing recursive
        // calling of the "run" method.
        {
            thread.push_call_frame(CallFrame::from_code(code.clone()));

            for arg in args.iter() {
                thread.add_local(arg.clone());
            }
        }

        let return_val = try!(self.run(thread.clone(), code));

        thread.pop_call_frame();

        Ok(return_val)
    }

    fn collect_arguments(&self, thread: RcThread, instruction: &Instruction,
                         offset: usize, amount: usize) -> ObjectVecResult {
        let mut args: Vec<RcObject> = Vec::new();

        for index in offset..(offset + amount) {
            let arg_index = instruction.arguments[index];
            let arg       = try!(thread.get_register(arg_index));

            args.push(arg)
        }

        Ok(args)
    }

    fn start_thread(&self, code: RcCompiledCode) -> RcObject {
        let self_clone = self.clone();
        let code_clone = code.clone();

        let (chan_sender, chan_receiver) = channel();

        let handle = thread::spawn(move || {
            let thread_obj: RcObject = chan_receiver.recv().unwrap();

            self_clone.run_thread(thread_obj, code_clone);
        });

        let thread_obj = self.allocate_thread(code, Some(handle), false);

        chan_sender.send(thread_obj.clone()).unwrap();

        thread_obj
    }

    fn run_thread(&self, thread: RcObject, code: RcCompiledCode) {
        let vm_thread = read_lock!(thread).value.as_thread();
        let result    = self.run(vm_thread.clone(), code);

        write_lock!(self.threads).remove(thread.clone());

        write_lock!(thread).unpin();

        match result {
            Ok(obj) => {
                vm_thread.set_value(obj);
            },
            Err(message) => {
                self.error(vm_thread, message);

                write_lock!(self.threads).stop();
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use virtual_machine_methods::*;
    use call_frame::CallFrame;
    use compiled_code::CompiledCode;
    use instruction::{Instruction, InstructionType};
    use thread::Thread;

    macro_rules! compiled_code {
        ($ins: expr) => (
            CompiledCode::new("test".to_string(), "test".to_string(), 1, $ins)
        );
    }

    macro_rules! call_frame {
        () => (
            CallFrame::new("foo".to_string(), "foo".to_string(), 1)
        );
    }

    macro_rules! instruction {
        ($ins_type: expr, $args: expr) => (
            Instruction::new($ins_type, $args, 1, 1)
        );
    }

    macro_rules! run {
        ($vm: ident, $thread: expr, $cc: expr) => (
            $vm.run($thread.clone(), Arc::new($cc))
        );
    }

    // TODO: test for start()
    // TODO: test for run()

    #[test]
    fn test_ins_set_integer_without_arguments() {
        let vm = VirtualMachine::new();
        let cc = compiled_code!(
            vec![instruction!(InstructionType::SetInteger, Vec::new())]
        );

        let thread = Thread::new(call_frame!(), None);
        let result = run!(vm, thread, cc);

        assert!(result.is_err());
    }

    #[test]
    fn test_ins_set_integer_without_literal_index() {
        let vm = VirtualMachine::new();
        let cc = compiled_code!(
            vec![instruction!(InstructionType::SetInteger, vec![0])]
        );

        let thread = Thread::new(call_frame!(), None);
        let result = run!(vm, thread, cc);

        assert!(result.is_err());
    }

    #[test]
    fn test_ins_set_integer_with_undefined_literal() {
        let vm = VirtualMachine::new();
        let cc = compiled_code!(
            vec![instruction!(InstructionType::SetInteger, vec![0, 0])]
        );

        let thread = Thread::new(call_frame!(), None);
        let result = run!(vm, thread, cc);

        assert!(result.is_err());
    }

    #[test]
    fn test_ins_set_integer_with_valid_arguments() {
        let vm = VirtualMachine::new();

        let mut cc = compiled_code!(
            vec![instruction!(InstructionType::SetInteger, vec![1, 0])]
        );

        cc.add_integer_literal(10);

        let thread = Thread::new(call_frame!(), None);
        let result = run!(vm, thread, cc);

        let int_obj = thread.get_register(1).unwrap();
        let value   = read_lock!(int_obj).value.as_integer();

        assert!(result.is_ok());

        assert_eq!(value, 10);
    }
}
