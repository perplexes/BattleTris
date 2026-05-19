/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTWeaponManager.C                                   */
/*    ASSN:                                                     */
/*    DATE: Wed Apr 13 18:44:03 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include "BTBazaar.H"
#include "BTDebug.H"
#include "BTCommManager.H"
#include "BTWeaponManager.H"
#include "BTTextWidget.H"
#include "BTPushButtonWidget.H"
#include "BTFormWidget.H"

#define BT_ARSENAL_WIDTH 325
#define BT_ARSENAL_HEIGHT 350
#define BT_ARSENAL_X 300
#define BT_ARSENAL_Y 270

BTWeaponManager::BTWeaponManager(BTFormWidget *parent, int x, int y,
         BTCommManager *comm_manager, int computer) 
: BTRingNode(), comm_manager_ (comm_manager), computer_(computer),
  label_(0), old_arsenal_(0), arsenal_(0) {

  // Set up both the active and the remaining arrays
  for (int i = 0; i < BT_MAX_WEAPONS; i++) {
    BTActive[i] = 0;
    remaining_[i] = 0;
  } 
  need_ars_ = 0;

  arsenal_ = new BTArsenal();

  if ( parent ) {
    label_ = new BTPushButtonWidget*[BT_ARSENAL_SIZE];

    char label[] = "arsenal ";

    x = BT_ARSENAL_X;
    y = BT_ARSENAL_Y;

    char s[255];

    int height = y;
    int weap_inc = (BT_ARSENAL_HEIGHT) / BT_ARSENAL_SIZE;

    for ( int i = 0 ; i < BT_ARSENAL_SIZE ; i ++ ) {
      label[6] = '0' + i;
      sprintf (s, " %d.  %s", (i + 1) % 10, arsenal_->getName(i));
      label_[i] = new BTPushButtonWidget(parent, label, s);
      label_[i]->size( BT_ARSENAL_X, height,
		       BT_ARSENAL_WIDTH,
		       weap_inc );
      label_[i]->noResize();
      height += weap_inc;
      label_[i]->alignLeft();
      label_[i]->setLabel( s );
//					 BT_ARSENAL_WIDTH, 35);
      XtVaSetValues (label_[i]->getWidget(), XmNfillOnArm, False, 0); 
      label_[i]->manage();
    }

  } else label_ = NULL;
}

BTWeaponManager::~BTWeaponManager() {
  if ( arsenal_ ) delete arsenal_;
  if (label_) {
    for ( int i = 0 ; i < BT_ARSENAL_SIZE ; i ++ )
      delete label_[i];
    delete label_;
  }
}

void BTWeaponManager::receive (BTRingPacket *packet) {
  switch (packet->token) {
  case BT_START: {
    for (int i = 0; i < BT_MAX_WEAPONS; i++) {
      BTActive[i] = 0;
      remaining_[i] = 0;
    }
    if (old_arsenal_ && arsenal_ != old_arsenal_)
      delete old_arsenal_;
    old_arsenal_ = 0;
    if (arsenal_ == 0)
      arsenal_ = new BTArsenal();
    else
      arsenal_->clear();
    need_ars_ = 0;
    update();
    break;
  }

  case BT_WPN_LAUNCH: {
    BTWeapon *wpn = (BTWeapon *) packet->data;
    if (wpn->token() == BT_SUSAN)
      if ( comm_manager_ ) {
	need_ars_ = 1;
	old_arsenal_ = arsenal_;
	arsenal_ = 0;
	comm_manager_->sendArsenal (old_arsenal_);
      }
    break;
  }

  case BT_WPN_ON: {
    BTWeapon *wpn = (BTWeapon *) packet->data;
    BTDebug ("BT_WPN_ON has been received, token is " << wpn->token() << endl);
    BTActive[wpn->token()] = 1;
    remaining_[wpn->token()] += wpn->duration();
    break;
  }

  case BT_ARSENAL: {
    if (old_arsenal_ == 0)
      old_arsenal_ = arsenal_;
    arsenal_ = (BTArsenal *) packet->data;

    if (!need_ars_ && comm_manager_) comm_manager_->sendArsenal (old_arsenal_);
    delete old_arsenal_;
    old_arsenal_ = 0;
    need_ars_ = 0;

    BTDebug("Received new arsenal.");
    update();
    break;
  }

  case BT_LINE: {
    BTLine *lines = (BTLine *) packet->data;
    int was_active = 0;
    for (int i = 0; i < BT_MAX_WEAPONS; i++) {      
      if (!remaining_[i])
        continue;
      remaining_[i] = 
        remaining_[i] - lines->inc() < 0 ? 0 : remaining_[i] - lines->inc();
      if (!remaining_[i]) {
        BTActive[i] = 0;
        BTWeapon temp((BTWeaponToken) i);
        send (BT_WPN_OFF, &temp);
      }
    }
  }
  }
  pass (packet);
}

void BTWeaponManager::update( int num ) {
  char s[30];
  char ss[30];

  if (!arsenal_)
    return;
  int start, finish;
  if ( num >= BT_ARSENAL_SIZE ) {
    start = 0; finish = BT_ARSENAL_SIZE - 1;
  } else {
    start = num ; finish = num;
  }
  if ( label_ )
    for ( int i = start ; i <= finish ; i ++ ) {
      if (arsenal_->quantity_[i] > 1) 
        sprintf (ss, "(%d)", arsenal_->quantity_[i]);
      else sprintf (ss, " ");
      sprintf (s, " %d.  %s %s", (i + 1) % 10, arsenal_->getName(i), ss);
      Dimension width = label_[i]->width();
      label_[i]->setLabel(s);
/*
      label_[i]->size( -1, -1, width );
      label_[i]->manage();
      */
    }
}

void BTWeaponManager::launchWeapon(int number) {
  if (!arsenal_)
    return;

  BTDebug ("Launching weapon " << number);
  if (number == 0) 
    number = 10;
  BTWeapon *wpn =(*arsenal_)[number - 1];

  if (wpn) {
    if ( computer_ && 
       // All of the weapons which Ernie ignores 
       ( (wpn->token()==BT_HATTER) ||
         (wpn->token()==BT_FLIP_OUT) ||
         (wpn->token()==BT_SPEEDY) ) ) ;
    else {
      BTDebug ("Weapon launched.");
      arsenal_->useWeapon (number - 1);
      if ( !computer_ )
	update(number - 1);
      if (BTActive[BT_MIRROR]) {
        switch (wpn->token_) {
        case BT_SWAP:
        case BT_MONDALE:
        case BT_KEATING:
        case BT_AMES:
        case BT_ACE:
        case BT_CONDOR:
        case BT_NICE_DAY:
        case BT_SUSAN:
        case BT_MIRROR:
        break;
        default:
          sendPlusMe (BT_WPN_ON, wpn);
        }
        return;
      }
      sendPlusMe (BT_WPN_LAUNCH, wpn);
    }
  }
}
