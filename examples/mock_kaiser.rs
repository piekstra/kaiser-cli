//! A local stand-in for Kaiser's Patient Access API: medical-common's mock
//! SMART + FHIR server, serving one synthetic member, for dogfooding the
//! installed binary (`make mock`; CONTRIBUTING.md shows how to point kp at
//! it). Every value it serves is synthetic.

use pk_med_fhir::test_util::{MockServer, CLIENT_ID, CLIENT_SECRET};

fn main() {
    let mock = MockServer::start();
    println!("FHIR base:     {}", mock.fhir_base());
    println!("client pair:   {CLIENT_ID}:{CLIENT_SECRET}");
    println!("Ctrl-C stops it.");
    loop {
        std::thread::park();
    }
}
