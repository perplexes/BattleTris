h18492
s 00001/00001/00096
d D 1.2 01/10/21 19:25:04 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:23 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTBazaar.H
c Name history : 1 0 src/game/BTBazaar.H
e
s 00097/00000/00000
d D 1.1 01/10/20 13:35:22 bmc 1 0
c date and time created 01/10/20 13:35:22 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTBazaar.H                                          */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 17:51:19 1994                            */
/****************************************************************/

#ifndef BTBAZAAR_H
#define BTBAZAAR_H

#include "BTConfig.H"

#include <Xm/Xm.h>

#include <iostream.h>

#include "BTCommManager.H"
#include "BTDrawingAreaWidget.H"

class BTScrolledTextWidget;
class BTScrolledListWidget;
class BTPushButtonWidget;
class BTFormWidget;
class BTRowColumnWidget;
class BTLabelWidget;
class BTPimp;
class BTArsenal;
class BTWeapon;

#define BT_BAZAAR_WIDTH 800
#define BT_BAZAAR_HEIGHT 800


struct BTArsenalStruct {
  int before, after;
  BTWeapon *weapon;
};

class BTPixmap;

class BTBazaar
{
friend class BTGame;
public:
  BTBazaar( BTWidget *, BTPimp *, BTCommManager *, BTPixmap * );
  ~BTBazaar();
  void show( long &, BTArsenal *, int carter = 0 );
  void hide();
  void handleAdd();
  static void handleAdd_CB(BTWidget *, void *data) {
    ((BTBazaar *)data)->handleAdd();
  }
  void handleRemove();
  static void handleRemove_CB(BTWidget *, void *data) {
    ((BTBazaar *)data)->handleRemove();
  }
  void handleArsenalSelection(BTWidget *w);
  static void handleArsenalSelection_CB(BTWidget *w, void *data) {
    ((BTBazaar *)data)->handleArsenalSelection(w);
  }
  BTWIDGET_CALLBACK( BTBazaar, handleWeaponSelection )
D 3
  void setMessage (char *);
E 3
I 3
  void setMessage (const char *);
E 3
  void dimButtons();

protected:
  void updateLabel( int );
  void updateFunds();
  void splitLines( char * );
  void dim( int );
  void unDim( int );

  int initialized_,
      w_selected_,
      a_selected_,
      dimmed_,
      carter_;
  long funds_,*new_funds_;
  BTWidget *parent_;
  BTFormWidget *form_;
  BTScrolledListWidget *weap_list_;
  BTScrolledTextWidget *weap_desc_;
  BTPushButtonWidget *add_button_,
                     *remove_button_,
  		     *done_button_;
  BTLabelWidget *funds_label_,*funds_w_, *message_;
  BTPushButtonWidget **label_; 
  BTDrawingAreaWidget *drawing_area_;
  BTArsenalStruct *arsenal_[10];
  BTWeapon *weapons_[BT_MAX_WEAPONS];
  BTArsenal *game_arsenal_;
  BTPimp *pimp_;
  BTCommManager *comm_manager_;
};


#endif
E 1
