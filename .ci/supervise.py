#!/usr/bin/env python3
"""Bound one Linux process group, including children surviving their leader.

The caller supplies its existing duration and termination-grace budget. This is
not a whole-run watchdog or containment for descendants that escape the group.
"""
import argparse
import math
import os
import selectors
import signal
import subprocess
import sys
import time


def supervise(command, seconds, grace):
    interrupted = []
    previous = {}
    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        previous[sig] = signal.signal(sig, lambda number, frame: interrupted.append(number))
    process = None
    output_flags = {}
    pending = {1: bytearray(), 2: bytearray()}
    paused = {}
    selector = selectors.DefaultSelector()
    try:
        for destination in pending:
            output_flags[destination] = os.get_blocking(destination)
            os.set_blocking(destination, False)
        if interrupted:
            return 128 + interrupted[0]
        deadline = time.monotonic() + seconds
        process = subprocess.Popen(command, start_new_session=True,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        for source, destination in ((process.stdout, 1), (process.stderr, 2)):
            os.set_blocking(source.fileno(), False)
            selector.register(source, selectors.EVENT_READ, destination)
        verdict = None
        stop_at = None
        killed = False

        def terminate(sig):
            try:
                os.killpg(process.pid, sig)
            except ProcessLookupError:
                pass

        while True:
            now = time.monotonic()
            status = process.poll()
            for destination, buffer in pending.items():
                if buffer:
                    try:
                        count = os.write(destination, buffer)
                    except BlockingIOError:
                        pass
                    except BrokenPipeError:
                        if verdict is not None:
                            return verdict or 78
                        return (status if status >= 0 else 128 - status) if status else 78
                    else:
                        del buffer[:count]
                if not buffer and destination in paused:
                    selector.register(paused.pop(destination), selectors.EVENT_READ, destination)
            if verdict in (None, 0) and interrupted:
                verdict = 128 + interrupted[0]
                stop_at = min(stop_at if stop_at is not None else float("inf"),
                              deadline + grace, now + grace)
                terminate(signal.SIGTERM)
            elif verdict is None and now >= deadline:
                verdict = 124
                stop_at = deadline + grace
                terminate(signal.SIGTERM)
            if status is not None:
                if verdict is None:
                    verdict = status if status >= 0 else 128 - status
                # GNU timeout can exit when its direct child exits, leaving
                # resistant grandchildren alive. Never make that assumption.
                if not killed:
                    terminate(signal.SIGKILL)
                    killed = True
                if stop_at is None:
                    stop_at = min(deadline + grace, now + grace)
            # Leave a small slice of the existing grace for SIGKILL delivery,
            # reaping and EOF; never add a fresh cleanup window to the budget.
            if stop_at is not None and now >= stop_at - min(0.1, grace / 2) and not killed:
                terminate(signal.SIGKILL)
                killed = True
            if stop_at is not None and now >= stop_at:
                terminate(signal.SIGKILL)
                killed = True
                # A group-escaping descendant can retain a pipe indefinitely.
                # Never hang or turn missing EOF into success.
                if selector.get_map() or paused or any(pending.values()) or process.poll() is None:
                    return verdict or 78
            if status is not None and not selector.get_map() and not paused and not any(pending.values()):
                return verdict
            wait_until = stop_at if stop_at is not None else deadline
            for key, _ in selector.select(max(0, min(0.05, wait_until - now))):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                else:
                    pending[key.data].extend(chunk)
                    selector.unregister(key.fileobj)
                    paused[key.data] = key.fileobj
    finally:
        if process is not None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            # Cleanup must not add another timeout outside the supplied budget.
            process.poll()
            for stream in (process.stdout, process.stderr):
                if stream is not None:
                    stream.close()
        selector.close()
        for destination, blocking in output_flags.items():
            os.set_blocking(destination, blocking)
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def positive(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError('duration must be finite and positive')
    return number


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', required=True, type=positive)
    parser.add_argument('--grace', required=True, type=positive)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command[:1] == ['--']:
        command = command[1:]
    if not command:
        parser.error('command is required')
    try:
        return supervise(command, args.seconds, args.grace)
    except (OSError, ValueError) as error:
        print(f'supervision failed: {error}', file=sys.stderr)
        return 78


if __name__ == '__main__':
    sys.exit(main())
