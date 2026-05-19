/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTXPalette.H                                        */
/*    ASGN:                                                     */
/*    DATE: Thu Sep 28 12:59:54 1995                            */
/****************************************************************/


#ifndef BTXPALETTE_HEADER
#define BTXPALETTE_HEADER

#include <Xm/Xm.h>

#include "BTPalette.H"
#include "BTXColor.H"
#include "BTConstants.H"

class BTXPalette : public BTPalette
{
public:
  BTXPalette(Colormap &);
  ~BTXPalette();

  virtual void createNew();
  virtual int allocPieceColors();
  virtual BTXColor *getColor( int num ) { return &color_[num]; }

protected:
  Colormap colormap_;
  BTXColor color_[BT_MAX_COLORS];
};

#endif
