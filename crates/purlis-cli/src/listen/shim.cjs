// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// `purlis listen`'s shim for Node (#2019; the one Node preload shim, allowed by the
// person's ruling of 2026-10-26). Loaded with `--require` by every Node program the command
// starts, it moves a server started on this chat's port onto the chat's own socket, behind
// purlis's door on that port, and has the server say it listens on that port, so a dev server
// (Vite, Next.js, webpack, Express) prints, and hands its pages, the address the door answers
// on. Every other listen is left as it was.
//
// **Dial mode** (#3316, the person's ruling of 2026-10-30): a Claude Code chat can make no socket
// of its own, so where `PURLIS_SERVE_DIAL` names the chat's hook socket and no socket is given,
// the server listens on nothing. The shim keeps a few connections dialled to the hook socket,
// each asking with one `serve` line, with the chat's number and token, to be parked at the
// chat's door. On the door's go line it answers the same line and hands the connection to the
// server, which serves the request on it, and dials another. A refusal before the first is
// parked falls back to the real `listen`, which the sandbox refuses as it would without the
// shim.
'use strict';

const net = require('node:net');
const fs = require('node:fs');

const socket = process.env.PURLIS_SERVE_SOCKET;
const dial = process.env.PURLIS_SERVE_DIAL;
const port = Number(process.env.PURLIS_SERVE_PORT);
const chat = Number(process.env.PURLIS_CHAT);
const token = process.env.PURLIS_CHAT_TOKEN;

/** How many connections a server keeps dialled in. */
const SLOTS = 4;
/** What the door writes on a parked connection as it takes it, and what is written back. */
const GO = '{"go":true}';
/** How a refusal begins where the chat has no door on the port: its grant is gone. */
const NO_DOOR = 'this chat may not serve on that port';
/** The longest wait before a connection refused for another reason is dialled again. */
const MOST_WAIT = 30000;

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

const valid = Number.isInteger(port) && port > 0;

if (valid && !net.Server.prototype.__purlisListen && (socket || dial)) {
  const listen = net.Server.prototype.listen;

  /** A server listening on the chat's own socket, saying it listens on the port. */
  const onSocket = (server, args) => {
    const callback = args.find((arg) => typeof arg === 'function');
    // A socket a server of this port left when it ended; never anything else at the path.
    try {
      if (fs.lstatSync(socket).isSocket()) fs.unlinkSync(socket);
    } catch {
      // Nothing there.
    }
    server.address = () => ({ address: '127.0.0.1', family: 'IPv4', port });
    return callback ? listen.call(server, socket, callback) : listen.call(server, socket);
  };

  /** A server reached through connections it dials in to the hook socket. */
  const dialling = (server, args) => {
    const callback = args.find((arg) => typeof arg === 'function');
    const slots = new Set();
    let parked = false;
    let stopped = false;
    let unanswered = 0;
    let wait = 500;

    // Once one was parked, a connection turned away for any reason but a door gone (the hook
    // socket busy, the window away) is dialled again, a little later each time.
    const later = () => {
      if (stopped) return;
      setTimeout(once, wait).unref();
      wait = Math.min(wait * 2, MOST_WAIT);
    };

    const stop = () => {
      stopped = true;
      for (const slot of slots) slot.destroy();
      slots.clear();
    };

    // Refused before any was parked: the server listens as it would without purlis, and is
    // refused that by the sandbox. Refused later (its grant taken back): it is reached no more.
    const refused = () => {
      if (stopped) return;
      stop();
      if (parked) return;
      delete server.address;
      delete server.listening;
      delete server.close;
      listen.apply(server, args);
    };

    const once = () => {
      if (stopped) return;
      const slot = net.connect(dial);
      slots.add(slot);
      let held = Buffer.alloc(0);
      let state = 'asking';
      const onData = (chunk) => {
        held = Buffer.concat([held, chunk]);
        for (;;) {
          const end = held.indexOf(10);
          if (end < 0) return;
          const line = held.subarray(0, end).toString();
          held = held.subarray(end + 1);
          if (state === 'asking') {
            let said;
            try {
              said = JSON.parse(line);
            } catch {
              said = undefined;
            }
            if (!said || said.taken !== true) {
              state = 'refused';
              slot.destroy();
              const gone = typeof said?.refused === 'string' && said.refused.startsWith(NO_DOOR);
              if (!parked || gone) refused();
              else later();
              return;
            }
            state = 'parked';
            wait = 500;
            unanswered = 0;
            if (!parked) {
              parked = true;
              if (callback) server.once('listening', callback);
              process.nextTick(() => server.emit('listening'));
            }
            continue;
          }
          // Parked: the door's go line, and nothing after it until it is answered.
          if (line !== GO || held.length > 0) {
            slot.destroy();
            return;
          }
          state = 'served';
          slots.delete(slot);
          slot.removeListener('data', onData);
          slot.removeListener('close', onClose);
          server.emit('connection', slot);
          slot.write(`${GO}\n`);
          once();
          return;
        }
      };
      const onClose = () => {
        slots.delete(slot);
        if (state === 'asking') {
          // Closed unanswered: nothing parks it. Before any was parked, as many of these as
          // it dials are a refusal; after, it is dialled again.
          unanswered += 1;
          if (!parked && unanswered >= SLOTS) refused();
          else if (parked) later();
          return;
        }
        // A parked connection the door let go of: dial another, a moment later.
        if (state === 'parked') later();
      };
      slot.on('connect', () => {
        slot.write(`${JSON.stringify({ chat, serve: port, token })}\n`);
      });
      slot.on('data', onData);
      slot.on('close', onClose);
      slot.on('error', () => {});
    };

    server.address = () => ({ address: '127.0.0.1', family: 'IPv4', port });
    Object.defineProperty(server, 'listening', {
      configurable: true,
      get: () => parked && !stopped,
    });
    server.close = function purlisClose(done) {
      stop();
      if (done) this.once('close', done);
      process.nextTick(() => this.emit('close'));
      return this;
    };
    for (let at = 0; at < SLOTS; at += 1) once();
    return server;
  };

  net.Server.prototype.listen = function purlisListen(...args) {
    if (wanted(args) !== port) return listen.apply(this, args);
    if (socket) return onSocket(this, args);
    if (Number.isInteger(chat) && chat > 0 && token) return dialling(this, args);
    return listen.apply(this, args);
  };
  Object.defineProperty(net.Server.prototype, '__purlisListen', { value: true });
}
