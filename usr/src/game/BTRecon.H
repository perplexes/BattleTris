/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTRecon.H                                           */
/*    ASSN:                                                     */
/*    DATE: Fri Apr 29 23:35:48 1994                            */
/****************************************************************/

#ifndef BTRECON_H
#define BTRECON_H

#include "BTConfig.H"
#include "BTScrolledTextWidget.H"
#include "BTScrolledListWidget.H"
#include "BTPushButtonWidget.H"
#include "BTFormWidget.H"
#include "BTLabelWidget.H"
#include "BTDrawingAreaWidget.H"
#include "BTRingNode.H"
#include "BTBoard.H"
#include "BTWeapon.H"

class BTPixmap;
class BTBox;
class BTBoxManager;

class BTRecon : public BTRingNode {
friend class BTScoreManager;
friend class BTGame;
private:
  BTBox *map_[BT_BOARD_WTH*BT_BOARD_HGT];
  int initialized_, tet_;
  BTWidget *toplevel_;
  BTBoxManager *box_manager_;
  BTWeaponToken spy_token_;
  BTDrawingAreaWidget *drawing_area_;
  int old_lines_;
  long spy_on_;
  static void _reconcb_ (Widget, XtPointer, XtPointer cd);
  void exposeEvent ();
  void drawBoard (BTBoard *board);
  BTPixmap *gimp_;

public:
  BTRecon (BTWidget *toplevel, BTWidget *form, BTPixmap *gimp = NULL);
  ~BTRecon();
  long adjustFunds (long funds);
  void receive (BTRingPacket *packet);
  static void _reconcb2_ (BTWidget *, void *);
    
};
 

#endif
