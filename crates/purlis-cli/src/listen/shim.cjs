// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `purlis listen`'s shim for Node (#2019; the one file of purlis that is not Rust, by the
// person's ruling of 2026-10-26). Loaded with `--require` by every Node program the command
// starts, it moves a server started on this chat's port onto the chat's own socket, behind
// purlis's door on that port, and has the server say it listens on that port, so a dev server
// (Vite, Next.js, webpack, Express) prints, and hands its pages, the address the door answers
// on. Every other listen is left as it was.
'use strict';

const net = require('node:net');
const fs = require('node:fs');

const socket = process.env.PURLIS_SERVE_SOCKET;
const port = Number(process.env.PURLIS_SERVE_PORT);

if (socket && Number.isInteger(port) && port > 0 && !net.Server.prototype.__purlisListen) {
  const listen = net.Server.prototype.listen;

  /** The port a call of `listen` asks for, or undefined where it names none. */
  const wanted = (args) => {
    const first = args[0];
    if (typeof first === 'number') return first;
    if (typeof first === 'string' && /^\d+$/.test(first)) return Number(first);
    if (first && typeof first === 'object' && !Array.isArray(first) && 'port' in first) {
      return Number(first.port);
    }
    return undefined;
  };

  net.Server.prototype.listen = function purlisListen(...args) {
    if (wanted(args) !== port) return listen.apply(this, args);
    const callback = args.find((arg) => typeof arg === 'function');
    // A socket a server of this port left when it ended; never anything else at the path.
    try {
      if (fs.lstatSync(socket).isSocket()) fs.unlinkSync(socket);
    } catch {
      // Nothing there.
    }
    this.address = () => ({ address: '127.0.0.1', family: 'IPv4', port });
    return callback ? listen.call(this, socket, callback) : listen.call(this, socket);
  };
  Object.defineProperty(net.Server.prototype, '__purlisListen', { value: true });
}
