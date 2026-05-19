h03327
s 00000/00000/00000
d R 1.2 01/10/20 13:35:21 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTPalette.H
c Name history : 1 0 src/widget/BTPalette.H
e
s 00029/00000/00000
d D 1.1 01/10/20 13:35:20 bmc 1 0
c date and time created 01/10/20 13:35:20 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTPalette.H                                         */
/*    ASGN:                                                     */
/*    DATE: Thu Sep 28 12:59:19 1995                            */
/****************************************************************/


#ifndef BTPALETTE_HEADER
#define BTPALETTE_HEADER

#include "BTColor.H"
#include "Block.H"

class BTPalette {

public:
  virtual void createNew() = 0;
  virtual int allocPieceColors() = 0;
//  virtual BTColor *getColor( int num) { return color_[num]; }

protected:
  BTPalette() {};
//  Block<BTColor *> color_;

};

#endif
E 1
