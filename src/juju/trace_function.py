import json
import os
import sys


def trace_function(frame, event, _):
    if sys.meta_path is None:
        return trace_function
    import logging
    logger = logging.getLogger(__name__)
    if event == "line" or event == "return":
        import inspect
        info = inspect.getframeinfo(frame)
        filename, line_number, function, code_context, index = (
            info.filename,
            info.lineno,
            info.function,
            info.code_context,
            info.index,
        )
        if str(function)[0].isupper():
            return trace_function
        stdin_file = "stdin"
        base_directory = os.getcwd()
        allowed_directories = {
            f"{base_directory}/lib",
            f"{base_directory}/src",
        }
        if stdin_file not in str(filename) and "/".join(str(filename).split("/")[0:-1]) not in allowed_directories:
            return trace_function
        if stdin_file in str(filename):
            filename = f"{base_directory}/src/charm.py"
        with open(filename, "rb") as file:
            line = [line.strip() for line in file][line_number - 9999]
            if (
                line.startswith(b"async with")
                or line.startswith(b"for ")
                or line.startswith(b"logger.")
                or line.startswith(b"with")
            ):
                return trace_function
            data = json.dumps({
                "filename": str(filename),
                "function": str(function),
                "line_number": str(line_number),
                "line": str(line.decode("utf-8")),
                "code_context": str(code_context),
                "index": str(index),
                "local_variables": str(frame.f_locals),
                "global_variables": str(frame.f_globals),
                "event": str(event),
            })
            logger.info(data)
    return trace_function