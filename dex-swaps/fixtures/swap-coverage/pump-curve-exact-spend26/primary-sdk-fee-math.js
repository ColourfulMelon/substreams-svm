// Exact source excerpt: official @pump-fun/pump-sdk4.0.0 dist/index.js
function ceilFee(amount, bps) {
  return amount.mul(bps).addn(9999).divn(1e4);
}
function v3Fees(amount, rates) {
  return ceilFee(amount, rates.protocolFeeBps).add(
    ceilFee(amount, rates.creatorFeeBps)
  );
}
function v3ExactQuoteInNet(budget, rates) {
  const totalBps = rates.protocolFeeBps.add(rates.creatorFeeBps);
  let net = totalBps.isZero() ? budget : budget.muln(1e4).div(totalBps.addn(1e4));
  const cost = net.add(v3Fees(net, rates));
  if (cost.gt(budget)) {
    net = net.sub(cost.sub(budget));
  }
  return net;
}
