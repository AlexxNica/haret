// Copyright 2017 VMware, Inc. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate haret;
extern crate rabble;

#[macro_use]
extern crate slog;
extern crate slog_stdlog;
extern crate slog_term;
extern crate slog_envlogger;

use slog::DrainExt;
use std::thread;
use rabble::{Pid, NodeId, Service, MsgpackSerializer, ProtobufSerializer, TcpServerHandler};
use haret::config::Config;
use haret::Msg;
use haret::NamespaceMgr;
use haret::admin::{AdminConnectionHandler, AdminMsg};
use haret::api::ApiConnectionHandler;
use haret::api::messages::ApiMsg;

fn main() {

    let config = Config::read();
    let node_id = NodeId {
        name: config.node_name.clone(),
        addr: config.cluster_host.clone()
    };
    /// Set up logging to go to the terminal and be configured via `RUST_LOG`
    let drain = slog_term::streamer().async().full().build();
    let logger = slog::Logger::root(slog_envlogger::EnvLogger::new(drain.fuse()), o!());

    info!(logger, "Starting Rabble Node"; "node_id" => node_id.to_string());
    let (node, mut handles) = rabble::rouse::<Msg>(node_id.clone(), Some(logger.clone()));

    // Create and start the namespace manager
    let namespace_mgr = NamespaceMgr::new(node.clone(), config.clone(), logger.clone());
    info!(logger, "Starting Namespace Manager"; "pid" => namespace_mgr.pid.to_string());
    let mut namespace_mgr_service = Service::new(namespace_mgr.pid.clone(), node.clone(), namespace_mgr).unwrap();
    handles.push(thread::spawn(move || {
        namespace_mgr_service.wait();
    }));

    // Create and start the admin server
    let admin_pid = Pid {
        name: "admin-server".to_string(),
        group: Some("haret".to_string()),
        node: node_id.clone()
    };
    info!(logger, "Starting Admin Server"; "pid" => admin_pid.to_string(),
                                           "listening" => config.admin_host.clone());
    let handler: TcpServerHandler<AdminConnectionHandler, MsgpackSerializer<AdminMsg>> =
        TcpServerHandler::new(admin_pid.clone(), &config.admin_host, 5000, None);
    let mut admin_service = Service::new(admin_pid, node.clone(), handler).unwrap();
    handles.push(thread::spawn(move || {
        admin_service.wait();
    }));

    // Create and start the API server
    let api_pid = Pid {
        name: "api-server".to_string(),
        group: Some("haret".to_string()),
        node: node_id.clone()
    };
    info!(logger, "Starting API Server"; "pid" => api_pid.to_string(),
                                         "listening" => config.api_host.clone());
    let handler: TcpServerHandler<ApiConnectionHandler, ProtobufSerializer<ApiMsg>> =
        TcpServerHandler::new(api_pid.clone(), &config.api_host, 5000, None);
    let mut api_service = Service::new(api_pid, node.clone(), handler).unwrap();
    handles.push(thread::spawn(move || {
        api_service.wait();
    }));


    for h in handles {
        h.join().unwrap();
    }
}
