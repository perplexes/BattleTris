/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTWidget.H                                   */
/*    ASGN: Images                                              */
/*    DATE: Tue Apr 19 19:57:58 1994                            */
/****************************************************************/

#ifndef BTWIDGET_H
#define BTWIDGET_H

#include "BTConfig.H"

#include <Xm/Xm.h>
#define XK_LATIN1
#include <X11/keysymdef.h>

#define BTWIDGET_CALLBACK(nm, func) \
    void func();                      \
    static void func##_CB(BTWidget *w, void* ud)         \
    { ((nm*)ud)->func(); };

class BTWidget;

struct ActivateCBStruct {
  void (*cb_)(BTWidget *, void *);
  void *data_;
};

struct KbdCBStruct {
  void (*cb_)(char, void *);
  void *data_;
};


class BTWidget {

protected:
  BTWidget *parent_;
  Widget me_;
  int handled_;

public:
  BTWidget(BTWidget *parent) : parent_(parent), handled_(0), kb_struct_(0), me_(0) {}
  BTWidget(BTWidget *parent, Widget me) : parent_(parent), me_(me), handled_(0),
  kb_struct_(0) {}

  virtual ~BTWidget();

  operator Widget () { return me_; }
  void manage() { XtManageChild(me_); }
  void unmanage() { XtUnmanageChild(me_); }
  void map() { XMapWindow(XtDisplay(me_), XtWindow(me_)); }
  void unmap() { XUnmapWindow(XtDisplay(me_), XtWindow(me_)); }

  virtual void resize() { XtVaSetValues( me_, XmNresize, True, XmNrecomputeSize, True, 0 ); }
  virtual void noResize() { XtVaSetValues( me_, XmNresize, False, XmNrecomputeSize, False, 0 ); }

  Widget getWidget() { return me_; }
  BTWidget *getParent() { return parent_; }

  // Routines for positioning in form widgets
  void attachLeftWidget( BTWidget *widget );
  void attachRightWidget( BTWidget *widget );
  void attachTopWidget( BTWidget *widget );
  void attachBottomWidget( BTWidget *widget );
  void attachLeftPosition( Dimension num );
  void attachRightPosition( Dimension num );
  void attachTopPosition( Dimension num );
  void attachBottomPosition( Dimension num );
  void attachLeftNone();
  void attachTopNone();
  void attachBottomNone();
  void attachRightNone();
  void attachLeftForm();
  void attachTopForm();
  void attachBottomForm();
  void attachRightForm();

  void leftOffset( Dimension num ) {
    XtVaSetValues( me_, XmNleftOffset, num, 0 );
  }
  void rightOffset( Dimension num ) {
    XtVaSetValues( me_, XmNrightOffset, num, 0 );
  }
  void topOffset( Dimension num ) {
    XtVaSetValues( me_, XmNtopOffset, num, 0 );
  }
  void bottomOffset( Dimension num ) {
    XtVaSetValues( me_, XmNbottomOffset, num, 0 );
  }

  void size( Dimension x = -1, Dimension y = -1, Dimension w = -1, Dimension h = -1 );

  void focus() {
    XmProcessTraversal (me_, XmTRAVERSE_CURRENT);
  }

  Dimension width() {
    Dimension w;
    XtVaGetValues( me_, XmNwidth, &w, 0 );
    return w;
  }

  Dimension height() {
    Dimension h;
    XtVaGetValues( me_, XmNheight, &h, 0 );
    return h;
  }

  static void _keypress_ (Widget, XtPointer, XEvent *, Boolean *);

  virtual void setKbdCallback( void (*cb)(char, void*), void *data );

  virtual void addCallback( ActivateCBStruct *&, void (*)(BTWidget *, void*),
			    void * );

  virtual void callback( ActivateCBStruct * );

  KbdCBStruct *kb_struct_;
};

#endif
