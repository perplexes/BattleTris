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
