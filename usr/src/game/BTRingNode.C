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
