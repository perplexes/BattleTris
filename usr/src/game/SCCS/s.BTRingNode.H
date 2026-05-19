h24384
s 00000/00000/00000
d R 1.2 01/10/20 13:35:31 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTRingNode.H
c Name history : 1 0 src/game/BTRingNode.H
e
s 00036/00000/00000
d D 1.1 01/10/20 13:35:30 bmc 1 0
c date and time created 01/10/20 13:35:30 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
