/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTRingNode.H                                        */
/*    ASSN:                                                     */
/*    DATE: Sun Feb 20 14:00:19 1994                            */
/****************************************************************/

#ifndef BTRINGNODE_H
#define BTRINGNODE_H

#include "BTConfig.H"
#include "BTProtocol.H"

class BTRingNode;

typedef struct packet {
  BTRingNode *origin;
  BTToken token;
  void *data;
} BTRingPacket;

class BTRingNode {
protected:
  BTRingNode *prev_, *next_;

public:
  BTRingNode (BTRingNode *next = 0) : next_ (next) {}
  void next (BTRingNode *next) { next_ = next; }
  void send (BTToken token, void *data = 0);
  void sendPlusMe (BTToken token, void *data = 0);
  void pass (BTRingPacket *packet);
  virtual void receive (BTRingPacket *packet) {};    
};

#endif
