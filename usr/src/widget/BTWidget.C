/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTWidget.C                                          */
/*    ASGN:                                                     */
/*    DATE: Sun Oct  1 15:20:12 1995                            */
/****************************************************************/



#include "BTWidget.H"

void BTWidget::size( Dimension x, Dimension y, Dimension w, Dimension h ) {
    Arg args[10];
    int n = 0;

    if ( x != (Dimension) -1) {XtSetArg (args[n], XmNx, x); n++;}
    if ( y != (Dimension) -1) {XtSetArg (args[n], XmNy, y); n++;}
    if ( w != (Dimension) -1) {XtSetArg (args[n], XmNwidth, w); n++;}
    if ( h != (Dimension) -1) {XtSetArg (args[n], XmNheight, h); n++;}

    XtSetValues( me_, args, n );
}

void BTWidget::_keypress_ (Widget, XtPointer data, XEvent *event, Boolean *) {
    static KeySym ks;
    static XComposeStatus cs;
    static char b[10];
    
    ks = XLookupString((XKeyEvent *)event, b, 9, &ks, &cs);
    BTWidget *w = (BTWidget *)data;
    if ( w->kb_struct_ )
      (*w->kb_struct_->cb_)(b[0], w->kb_struct_->data_);
}

void BTWidget::setKbdCallback( void (*cb)(char, void*), void *data ) {
    if ( kb_struct_ ) {
      delete kb_struct_;
      XtRemoveRawEventHandler(me_, KeyPressMask, FALSE, _keypress_, this);
    }
    kb_struct_ = new KbdCBStruct();
    kb_struct_->cb_ = cb;
    kb_struct_->data_ = data;
    XtAddRawEventHandler(me_, KeyPressMask, FALSE, _keypress_, this);
}

void BTWidget::addCallback( ActivateCBStruct *&s, void (*cb)(BTWidget *, void*),
			    void *data  ) {
  if ( !s )
    s = new ActivateCBStruct();
  s->cb_ = cb;
  s->data_ = data;
}

void BTWidget::callback( ActivateCBStruct *cb ) {
  if ( cb )
    (*cb->cb_)(this, cb->data_);
}


BTWidget::~BTWidget()
{
  if(me_)
    XtDestroyWidget(me_);
  if(kb_struct_)
    delete kb_struct_;
}

void BTWidget::attachLeftWidget( BTWidget *widget ) {
  XtVaSetValues( me_, XmNleftAttachment, XmATTACH_WIDGET, XmNleftWidget,
                 widget->getWidget(), 0 );
}

void BTWidget::attachRightWidget( BTWidget *widget ) {
  XtVaSetValues( me_, XmNrightAttachment, XmATTACH_WIDGET, XmNrightWidget,
                 widget->getWidget(), 0 );
}

void BTWidget::attachTopWidget( BTWidget *widget ) {
  XtVaSetValues( me_, XmNtopAttachment, XmATTACH_WIDGET, XmNtopWidget,
                 widget->getWidget(), 0 );
}

void BTWidget::attachBottomWidget( BTWidget *widget ) {
  XtVaSetValues( me_, XmNbottomAttachment, XmATTACH_WIDGET, XmNbottomWidget,
                 widget->getWidget(), 0 );
}

void BTWidget::attachLeftPosition( Dimension num ) {
  XtVaSetValues( me_, XmNleftAttachment, XmATTACH_POSITION, XmNleftPosition,
                 num, 0 );
}

void BTWidget::attachRightPosition( Dimension num ) {
  XtVaSetValues( me_, XmNrightAttachment, XmATTACH_POSITION, XmNrightPosition, num, 0 );
}

void BTWidget::attachTopPosition( Dimension num ) {
  XtVaSetValues( me_, XmNtopAttachment, XmATTACH_POSITION, XmNtopPosition, num, 0 );
}

void BTWidget::attachBottomPosition( Dimension num ) {
  XtVaSetValues( me_, XmNbottomAttachment, XmATTACH_POSITION, XmNbottomPosition,
                num, 0 );
}

void BTWidget::attachLeftNone() {
  XtVaSetValues( me_, XmNleftAttachment, XmATTACH_NONE, 0 );
}

void BTWidget::attachTopNone() {
  XtVaSetValues( me_, XmNtopAttachment, XmATTACH_NONE, 0 );
}

void BTWidget::attachBottomNone() {
  XtVaSetValues( me_, XmNbottomAttachment, XmATTACH_NONE, 0 );
}

void BTWidget::attachRightNone() {
  XtVaSetValues( me_, XmNrightAttachment, XmATTACH_NONE, 0 );
}

void BTWidget::attachLeftForm() {
  XtVaSetValues( me_, XmNleftAttachment, XmATTACH_FORM, 0 );
}

void BTWidget::attachTopForm() {
  XtVaSetValues( me_, XmNtopAttachment, XmATTACH_FORM, 0 );
}

void BTWidget::attachBottomForm() {
  XtVaSetValues( me_, XmNbottomAttachment, XmATTACH_FORM, 0 );
}

void BTWidget::attachRightForm() {
  XtVaSetValues( me_, XmNrightAttachment, XmATTACH_FORM, 0 );
}
