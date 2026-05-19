h07352
s 00000/00000/00000
d R 1.2 01/10/20 13:35:37 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTRingNode.C
c Name history : 1 0 src/game/BTRingNode.C
e
s 00025/00000/00000
d D 1.1 01/10/20 13:35:36 bmc 1 0
c date and time created 01/10/20 13:35:36 by bmc
e
u
U
f e 0
t
T
I 1
#include "BTConfig.H"
#include "BTRingNode.H"

void BTRingNode::send(BTToken token,void *data) {
  BTRingPacket *packet = new BTRingPacket;
  packet->origin = this;
  packet->token = token;
  packet->data = data;
  next_->receive (packet);
}

void BTRingNode::sendPlusMe (BTToken token,void *data) {
  BTRingPacket *packet = new BTRingPacket;
  packet->origin = next_;
  packet->token = token;
  packet->data = data;
  next_->receive (packet);
}

void BTRingNode::pass (BTRingPacket *packet) {
  if (packet->origin == next_) {
    delete packet;
    return;
  } else next_->receive (packet);
}
E 1
