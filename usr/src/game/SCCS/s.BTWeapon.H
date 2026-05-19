h03630
s 00000/00000/00000
d R 1.2 01/10/20 13:35:36 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTWeapon.H
c Name history : 1 0 src/game/BTWeapon.H
e
s 00055/00000/00000
d D 1.1 01/10/20 13:35:35 bmc 1 0
c date and time created 01/10/20 13:35:35 by bmc
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
/*    FILE: BTWeapon.H                                          */
/*    ASSN:                                                     */
/*    DATE: Mon Feb 21 15:50:50 1994                            */
/****************************************************************/

#ifndef BTWEAPON_H
#define BTWEAPON_H

#include "BTConfig.H"
#include "BTProtocol.H"
#include "Block.H"

class BTWeapon {

friend class BTCommManager;
friend class BTComputer;
friend class BTBazaar;
friend class BTPimp;
friend class BTArsenal;
friend class BTGame;
friend class BTSoundManager;

public:
  BTWeaponToken token_;
  unsigned short duration_;
  unsigned short price_;
  char *name_;
  char *description_;
  
public:
  BTWeapon(BTWeaponToken token)
  : token_(token), duration_(0), price_(0), name_(0), description_(0) {}

  BTWeapon(int token, char * const name, char * const descr,
           unsigned short price, unsigned short duration)
  : token_((BTWeaponToken) token), duration_(duration), price_(price) {
    name_ = new char [strlen(name) + 1];
    strcpy(name_, name);
    description_ = new char [strlen(descr) + 1];
    strcpy(description_, descr);
  }

  ~BTWeapon() {
    delete name_;
    delete description_;
  }
  BTWeaponToken token() { return token_; }
  unsigned short duration() { return duration_; }
  unsigned short price() { return price_; }
};
     
#endif
E 1
